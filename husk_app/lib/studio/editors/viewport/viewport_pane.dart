import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/studio/editors/viewport/gizmo_overlay.dart';
import 'package:husk_app/studio/editors/viewport/skeleton_overlay.dart';
import 'package:husk_app/studio/editors/viewport/viewport_tool_strip.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/messages.dart';
import 'package:husk_app/ui/typing.dart';

// The renderer never draws more than this many pixels on its long side.
const double _maxRenderSide = 1920;

/// The viewport: a tool strip on top and the live render filling the rest.
/// The engine renders at the panel's size, so nothing is stretched.
/// Dragging orbits the camera (or poses and moves, depending on the tool),
/// the mouse wheel and the - and = keys zoom, and tapping a joint dot
/// selects it.
class ViewportPane extends StatefulWidget {
  const ViewportPane({super.key, required this.actions});

  final StudioActions actions;

  @override
  State<ViewportPane> createState() => _ViewportPaneState();
}

class _ViewportPaneState extends State<ViewportPane> {
  StudioController get _studio => widget.actions.studio;
  ViewportController get _viewport => widget.actions.viewport;

  int? _textureId;

  // The size the engine is rendering at right now, in pixels. The native
  // side starts at 1000 x 1000 until the first resize.
  Size _renderSize = const Size(1000, 1000);
  Size? _requestedSize;
  Timer? _resizeTimer;
  bool _firstResizeDone = false;

  // The joint being dragged with the Select tool (null when the drag orbits
  // the camera), and whether the drag moves the whole character instead.
  BigInt? _draggedJoint;
  bool _dragMovesCharacter = false;

  int? _gizmoAxis;
  (double, double, double) _gizmoStart = (0, 0, 0);
  double _gizmoStartParam = 0;

  (double, double, double) _ringStartRotation = (0, 0, 0);
  double _ringPreviousAngle = 0;
  double _ringTotal = 0;

  (double, double, double) _scaleStart = (1, 1, 1);
  List<(double, double, double)> _scaleAxes = const [];
  Offset _scaleOrigin = Offset.zero;
  double _scaleStartDistance = 0;
  double _scaleRatio = 1;

  // The arrow to light up: the one under the cursor, or the one dragged.
  final ValueNotifier<int?> _gizmoHighlight = ValueNotifier<int?>(null);

  final ValueNotifier<int> _overlayTick = ValueNotifier<int>(0);
  Timer? _overlayTimer;

  @override
  void initState() {
    super.initState();
    _loadTextureId();
    HardwareKeyboard.instance.addHandler(_handleKey);
    _overlayTimer = Timer.periodic(
      const Duration(milliseconds: 33),
      (_) => _overlayTick.value++,
    );
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleKey);
    _overlayTimer?.cancel();
    _resizeTimer?.cancel();
    _overlayTick.dispose();
    _gizmoHighlight.dispose();
    super.dispose();
  }

  Future<void> _loadTextureId() async {
    final id = await _viewport.textureId();
    if (!mounted) return;
    setState(() {
      _textureId = id;
    });
  }

  /// Works out the render size for the panel (physical pixels, capped) and
  /// asks the engine for it. The first request goes straight away; later
  /// ones wait until the panel has stopped changing.
  void _requestSize(Size paneSize, double pixelRatio) {
    var width = paneSize.width * pixelRatio;
    var height = paneSize.height * pixelRatio;
    final longest = math.max(width, height);
    if (longest > _maxRenderSide) {
      final shrink = _maxRenderSide / longest;
      width *= shrink;
      height *= shrink;
    }
    final wanted = Size(
      width.round().clamp(16, 4096).toDouble(),
      height.round().clamp(16, 4096).toDouble(),
    );
    if (wanted == _requestedSize) return;
    _requestedSize = wanted;
    _resizeTimer?.cancel();
    if (!_firstResizeDone) {
      _firstResizeDone = true;
      _applySize(wanted);
    } else {
      _resizeTimer = Timer(
        const Duration(milliseconds: 150),
        () => _applySize(wanted),
      );
    }
  }

  Future<void> _applySize(Size size) async {
    try {
      await _viewport.resize(size.width.toInt(), size.height.toInt());
    } on PlatformException catch (error) {
      debugPrint('Viewport resize failed: ${error.message}');
      return;
    }
    if (!mounted) return;
    setState(() {
      _renderSize = size;
    });
  }

  bool _handleKey(KeyEvent event) {
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) return false;
    // A text box has the keys: Delete, - and = belong to it.
    if (isTyping()) return false;
    final key = event.logicalKey;
    if (key == LogicalKeyboardKey.minus ||
        key == LogicalKeyboardKey.numpadSubtract) {
      _viewport.zoom(1.08); // zoom out
      return true;
    }
    if (key == LogicalKeyboardKey.equal ||
        key == LogicalKeyboardKey.add ||
        key == LogicalKeyboardKey.numpadAdd) {
      _viewport.zoom(1 / 1.08); // zoom in
      return true;
    }
    if (key == LogicalKeyboardKey.delete && event is KeyDownEvent) {
      widget.actions.deleteObject(context);
      return true;
    }
    return false;
  }
  void _handlePointerSignal(PointerSignalEvent event) {
    if (event is PointerScrollEvent) {
      // Scrolling down moves the camera away, scrolling up brings it closer.
      _viewport.zoom(event.scrollDelta.dy > 0 ? 1.08 : 1 / 1.08);
    }
  }

  void _handleTap(TapUpDetails details, double scale) {
    // Tapping a joint dot always selects it.
    final hit = _viewport.jointAt(details.localPosition, scale);
    if (hit != null) {
      _studio.selectBone(hit);
      return;
    }
    if (_studio.tool == StudioTool.placeBones) {
      showMessage(
        context,
        _studio.placeBoneAt(
          details.localPosition.dx / scale,
          details.localPosition.dy / scale,
        ),
      );
    } else {
      _studio.selectBone(null);
      if (_studio.tool == StudioTool.select ||
          _studio.tool == StudioTool.moveObject ||
          _studio.tool == StudioTool.rotateObject ||
          _studio.tool == StudioTool.scaleObject) {
        final picked = _studio.selectObjectAt(
          details.localPosition.dx / scale,
          details.localPosition.dy / scale,
        );
        // TEMPORARY feedback until the outliner shows the selection.
        if (picked != null) showMessage(context, 'Selected object $picked');
      }
    }
  }

  void _handlePanStart(DragStartDetails details, double scale) {
    _draggedJoint = null;
    _dragMovesCharacter = false;
    _gizmoAxis = null;
    switch (_studio.tool) {
      case StudioTool.move:
        _dragMovesCharacter = true;
      case StudioTool.select:
        final joint = _viewport.jointAt(details.localPosition, scale);
        _draggedJoint = joint;
        // A root joint has no parent to swing around: dragging it moves
        // the whole character.
        _dragMovesCharacter = joint != null && _viewport.isRoot(joint);
      case StudioTool.placeBones:
        break; // Dragging orbits the camera.
      case StudioTool.moveObject:
        // A drag that starts on an arrow moves the object; anywhere else
        // it orbits the camera.
        _beginGizmoDrag(details.localPosition, scale);
      case StudioTool.rotateObject:
        // Likewise for a ring: on it, the object turns; elsewhere, orbit.
        _beginRingDrag(details.localPosition, scale);
      case StudioTool.scaleObject:
        // And for a scale handle.
        _beginScaleDrag(details.localPosition, scale);
    }


  }

  void _handlePanUpdate(DragUpdateDetails details, double scale) {
    final x = details.localPosition.dx / scale;
    final y = details.localPosition.dy / scale;
    if (_gizmoAxis != null) {
      _updateGizmoDrag(x, y);
      return;
    }
    if (_dragMovesCharacter) {
      _viewport.moveCharacter(x, y);
      return;
    }
    final joint = _draggedJoint;
    if (joint == null) {
      // Degrees-per-pixel-dragged, converted to radians for the engine.
      const dragSensitivity = 0.01;
      _viewport.orbit(
        details.delta.dx * dragSensitivity,
        -details.delta.dy * dragSensitivity,
      );
      return;
    }
    _viewport.dragJoint(joint, x, y, ik: _studio.ikMode);
  }

  /// Starts a drag on a gizmo arrow if the pointer is on one; otherwise
  /// the drag orbits the camera as usual.
  void _beginGizmoDrag(Offset pointer, double scale) {
    final position = _studio.activeTransform.position;
    final axis = _viewport.gizmoAxisAt(pointer, position, scale, kGizmoLength);
    if (axis == null) return;
    final param = _viewport.axisParam(
      position,
      axis,
      pointer.dx / scale,
      pointer.dy / scale,
    );
    if (param == null) return;
    _gizmoAxis = axis;
    _gizmoStart = position;
    _gizmoStartParam = param;
    _gizmoHighlight.value = axis;
  }

  /// Starts a drag on a rotation ring if the pointer is on one and the ring
  /// can be read (a ring seen exactly edge-on cannot); otherwise the drag
  /// orbits the camera as usual.
  void _beginRingDrag(Offset pointer, double scale) {
    final position = _studio.activeTransform.position;
    final axis = _viewport.gizmoRingAt(
      pointer,
      position,
      scale,
      kGizmoRingRadius,
    );
    if (axis == null) return;
    final angle = _viewport.ringAngle(
      position,
      axis,
      pointer.dx / scale,
      pointer.dy / scale,
    );
    if (angle == null) return;
    _gizmoAxis = axis;
    _gizmoStart = position;
    _ringStartRotation = _studio.activeTransform.rotationDegrees;
    _ringPreviousAngle = angle;
    _ringTotal = 0;
    _gizmoHighlight.value = axis;
  }

  /// Turns the active object about the dragged ring's scene axis by how far
  /// the cursor has gone around it since the drag began.
  void _updateRingDrag(int axis, double x, double y) {
    final angle = _viewport.ringAngle(_gizmoStart, axis, x, y);
    // A ring seen edge-on has no angle: hold the rotation until it can be
    // read again.
    if (angle == null) return;
    // Adding up the small steps lets the turn go past half a circle.
    _ringTotal += _viewport.angleDifference(_ringPreviousAngle, angle);
    _ringPreviousAngle = angle;
    _studio.setActiveRotation(
      _viewport.rotatedEuler(_ringStartRotation, axis, _ringTotal),
    );
  }

  /// Starts a drag on a scale handle if the pointer is on one; otherwise
  /// the drag orbits the camera as usual. The arrows run along the object's
  /// own axes: scaling along a scene axis would shear a rotated object,
  /// which an object transform cannot hold.
  void _beginScaleDrag(Offset pointer, double scale) {
    final position = _studio.activeTransform.position;
    final axes = _studio.activeLocalAxes;
    final handle = _viewport.scaleHandleAt(
      pointer,
      position,
      scale,
      kGizmoLength,
      axes,
    );
    if (handle == null) return;
    final x = pointer.dx / scale;
    final y = pointer.dy / scale;
    if (handle == ViewportController.uniformScaleHandle) {
      final origin = _viewport.screenPoint(position);
      if (origin == null) return;
      _scaleOrigin = origin;
      _scaleStartDistance = (Offset(x, y) - origin).distance;
    } else {
      final param = _viewport.axisParam(
        position,
        handle,
        x,
        y,
        directions: axes,
      );
      // Grabbed too close to the object's origin to tell how far it was.
      if (param == null || param.abs() < 1e-3) return;
      _gizmoStartParam = param;
    }
    _gizmoAxis = handle;
    _gizmoStart = position;
    _scaleStart = _studio.activeTransform.scale;
    _scaleAxes = axes;
    _scaleRatio = scale;
    _gizmoHighlight.value = handle;
  }

  /// Scales the active object by how far the cursor has moved. On an arrow
  /// the scale on that axis grows and shrinks in proportion to the
  /// cursor's distance along it from the object's origin. On the centre
  /// box all three axes scale together.
  void _updateScaleDrag(int handle, double x, double y) {
    final (sx, sy, sz) = _scaleStart;
    if (handle == ViewportController.uniformScaleHandle) {
      final distance = (Offset(x, y) - _scaleOrigin).distance;
      // Every 100 panel pixels outward doubles the size, moving back in
      // shrinks it, and it never goes below 1 percent of where it began.
      final factor = math.max(
        0.01,
        1 + (distance - _scaleStartDistance) * _scaleRatio / 100,
      );
      _studio.setActiveScale((sx * factor, sy * factor, sz * factor));
      return;
    }
    final param = _viewport.axisParam(
      _gizmoStart,
      handle,
      x,
      y,
      directions: _scaleAxes,
    );
    if (param == null) return;
    final factor = param / _gizmoStartParam;
    _studio.setActiveScale((
      handle == 0 ? sx * factor : sx,
      handle == 1 ? sy * factor : sy,
      handle == 2 ? sz * factor : sz,
    ));
  }

  /// Moves the active object along the dragged arrow so the point that was
  /// grabbed stays under the cursor.
  void _updateGizmoDrag(double x, double y) {
    final axis = _gizmoAxis;
    if (axis == null) return;
    if (_studio.tool == StudioTool.rotateObject) {
      _updateRingDrag(axis, x, y);
      return;
    }
    if (_studio.tool == StudioTool.scaleObject) {
      _updateScaleDrag(axis, x, y);
      return;
    }
    final param = _viewport.axisParam(_gizmoStart, axis, x, y);
    if (param == null) return;
    final (dx, dy, dz) = ViewportController.gizmoAxisDirections[axis];
    final along = param - _gizmoStartParam;
    final (sx, sy, sz) = _gizmoStart;
    _studio.setActivePosition(
      (sx + dx * along, sy + dy * along, sz + dz * along),
    );
  }

  void _endDrag() {
    _draggedJoint = null;
    _gizmoAxis = null;
    _gizmoHighlight.value = null;
  }

  /// Lights up the gizmo arrow under the cursor while nothing is dragged.
  void _updateHover(Offset pointer, double scale) {
    if (_gizmoAxis != null) return;
    final position = _studio.activeTransform.position;
    switch (_studio.tool) {
      case StudioTool.moveObject:
        _gizmoHighlight.value =
            _viewport.gizmoAxisAt(pointer, position, scale, kGizmoLength);
      case StudioTool.rotateObject:
        _gizmoHighlight.value = _viewport.gizmoRingAt(
          pointer,
          position,
          scale,
          kGizmoRingRadius,
        );
      case StudioTool.scaleObject:
        _gizmoHighlight.value = _viewport.scaleHandleAt(
          pointer,
          position,
          scale,
          kGizmoLength,
          _studio.activeLocalAxes,
        );
      case StudioTool.select:
      case StudioTool.placeBones:
      case StudioTool.move:
        _gizmoHighlight.value = null;
    }
  }

  Widget _buildRender(BuildContext context, int textureId) {
    return LayoutBuilder(
      builder: (context, constraints) {
        _requestSize(
          constraints.biggest,
          MediaQuery.devicePixelRatioOf(context),
        );
        // From render pixels (what the engine reports) to panel pixels.
        final scale = constraints.maxWidth / _renderSize.width;
        final render = Listener(
          onPointerSignal: _handlePointerSignal,
          onPointerHover: (event) => _updateHover(event.localPosition, scale),
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapUp: (details) => _handleTap(details, scale),
            onPanStart: (details) => _handlePanStart(details, scale),
            onPanUpdate: (details) => _handlePanUpdate(details, scale),
            onPanEnd: (_) => _endDrag(),
            onPanCancel: _endDrag,
            child: Stack(
              fit: StackFit.expand,
              children: [
                Texture(textureId: textureId),
                IgnorePointer(
                  child: CustomPaint(
                    painter: SkeletonOverlayPainter(
                      viewport: _viewport,
                      studio: _studio,
                      renderWidth: _renderSize.width,
                      repaint: Listenable.merge([_overlayTick, _studio]),
                    ),
                  ),
                ),
                IgnorePointer(
                  child: CustomPaint(
                    painter: GizmoOverlayPainter(
                      viewport: _viewport,
                      studio: _studio,
                      renderWidth: _renderSize.width,
                      highlighted: _gizmoHighlight,
                      repaint: Listenable.merge([
                        _overlayTick,
                        _studio,
                        _gizmoHighlight,
                      ]),
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
        return DragTarget<String>(
          onAcceptWithDetails: (details) {
            // The tray uses the pointer anchor strategy, so the drop offset
            // is the pointer's global position. Make it local to the render
            // area, then convert panel pixels to render pixels.
            final box = context.findRenderObject() as RenderBox;
            final local = box.globalToLocal(details.offset);
            showMessage(
              context,
              _studio.addShape(details.data, local.dx / scale, local.dy / scale),
            );
          },
          builder: (context, candidate, rejected) => render,
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final textureId = _textureId;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ViewportToolStrip(actions: widget.actions),
        Expanded(
          child: Container(
            color: HuskColors.canvas,
            child: textureId == null
                ? const Center(
                    child: CircularProgressIndicator(color: HuskColors.orange),
                  )
                : _buildRender(context, textureId),
          ),
        ),
      ],
    );
  }
}
