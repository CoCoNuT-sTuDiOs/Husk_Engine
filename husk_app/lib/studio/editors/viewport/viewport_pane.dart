import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/studio/editors/tools/tool_column.dart';
import 'package:husk_app/studio/editors/viewport/frame_overlay.dart';

import 'package:husk_app/studio/editors/viewport/gizmo_overlay.dart';
import 'package:husk_app/studio/editors/viewport/maximized_bar.dart';
import 'package:husk_app/studio/editors/viewport/skeleton_overlay.dart';
import 'package:husk_app/studio/editors/viewport/viewport_tool_strip.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/messages.dart';



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

  // Why the viewport has no picture, when the graphics could not be started.
  String? _graphicsError;

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

  // Whether the drag in progress slides the active object over surfaces.
  bool _surfaceDrag = false;
  (double, double, double) _gizmoStart = (0, 0, 0);
  double _gizmoStartParam = 0;

  (double, double, double) _ringStartRotation = (0, 0, 0);
  double _ringPreviousAngle = 0;
  double _ringTotal = 0;

  (double, double, double) _scaleStart = (1, 1, 1);
  List<(double, double, double)> _scaleAxes = const [];
  Offset _scaleOrigin = Offset.zero;
  double _scaleRatio = 1;

  // The arrow to light up: the one under the cursor, or the one dragged.
  final ValueNotifier<int?> _gizmoHighlight = ValueNotifier<int?>(null);

  final ValueNotifier<int> _overlayTick = ValueNotifier<int>(0);
  Timer? _overlayTimer;

  @override
  void initState() {
    super.initState();
    _loadTextureId();
    _overlayTimer = Timer.periodic(
      const Duration(milliseconds: 33),
      (_) => _overlayTick.value++,
    );
  }

  @override
  void dispose() {
    _overlayTimer?.cancel();
    _resizeTimer?.cancel();
    _overlayTick.dispose();
    _gizmoHighlight.dispose();
    super.dispose();
  }

  Future<void> _loadTextureId() async {
        try {
      final id = await _viewport.textureId();
      if (!mounted) return;
      setState(() {
        _textureId = id;
      });
    } on PlatformException catch (error) {
      // The graphics could not be started (no usable GPU, say). Say so in
      // the viewport instead of showing a spinner for ever. The rest of
      // Husk still works.
      if (!mounted) return;
      setState(() {
        _graphicsError = error.message ?? 'The graphics could not be started.';
      });
    }
  }

  void _requestSize(Size paneSize, double pixelRatio) {
    final quality = _studio.quality;
    var width = paneSize.width * pixelRatio * quality.renderScale;
    var height = paneSize.height * pixelRatio * quality.renderScale;
    final longest = math.max(width, height);
    if (longest > quality.maxSide) {
      final shrink = quality.maxSide / longest;
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

  /// Dragging with the middle mouse button turns the view; with Shift held
  /// it slides the view instead, as in Blender.
  void _handleMiddleDrag(PointerMoveEvent event, double scale) {
    if (event.buttons != kMiddleMouseButton) return;
    if (HardwareKeyboard.instance.isShiftPressed) {
      _studio.leaveCameraView();
      // Panel pixels to render pixels, which is what the engine measures in.
      _viewport.pan(event.delta.dx / scale, event.delta.dy / scale);
      return;
    }
    // The same radians-per-pixel as the left-button orbit.
    const dragSensitivity = 0.01;
    _studio.leaveCameraView();
    _viewport.orbit(
      event.delta.dx * dragSensitivity,
      -event.delta.dy * dragSensitivity,
    );
  }

  void _handlePointerSignal(PointerSignalEvent event) {
    if (event is PointerScrollEvent) {
      // Scrolling down moves the camera away, scrolling up brings it closer.
      _studio.leaveCameraView();
      _viewport.zoom(event.scrollDelta.dy > 0 ? 1.08 : 1 / 1.08);
    }
  }

  void _handleTap(TapUpDetails details, double scale) {
        // In Pose Mode, or while placing bones (to pick a parent), tapping a joint
    // dot selects it. In Object Mode the joints cannot be selected.
    final jointsOn =
        _studio.mode == 'pose' || _studio.tool == StudioTool.placeBones;
    final hit =
        jointsOn ? _viewport.jointAt(details.localPosition, scale) : null;
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
         // Only Pose Mode poses joints; in Object Mode a drag orbits.
        final joint = _studio.mode == 'pose'
            ? _viewport.jointAt(details.localPosition, scale)
            : null;
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
        if (_gizmoAxis == null) _beginSurfaceDrag(details.localPosition, scale);
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
    if (_surfaceDrag) {
      _updateSurfaceDrag(details.localPosition, scale);
      return;
    }
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
      _studio.leaveCameraView();
      _viewport.orbit(
        details.delta.dx * dragSensitivity,
        -details.delta.dy * dragSensitivity,
      );
      return;
    }
    _viewport.dragJoint(joint, x, y, ik: _studio.ikMode);
  }

/// With Snap on, a drag that starts on the active object (not on an arrow)
  /// slides it over the surfaces under the cursor instead of orbiting.
  void _beginSurfaceDrag(Offset pointer, double scale) {
    if (!_studio.snapToSurface || _studio.activeLocked) return;
    final hit = _viewport.surfaceAt(pointer, scale, skipActive: false);
    if (hit == null || hit.object != _studio.objectList.$2) return;
    _surfaceDrag = true;
  }

  void _updateSurfaceDrag(Offset pointer, double scale) {
// Over empty space the object stays where it last was.
    _studio.snapActiveToSurface(pointer.dx / scale, pointer.dy / scale);
  }

  /// Starts a drag on a gizmo arrow if the pointer is on one; otherwise  /// the drag orbits the camera as usual.
  void _beginGizmoDrag(Offset pointer, double scale) {
    // A locked object has no handles, so the drag orbits instead.
    if (_studio.activeLocked) return;
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
    if (_studio.activeLocked) return;
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

  void _beginScaleDrag(Offset pointer, double scale) {
    if (_studio.activeLocked) return;
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
    _scaleOrigin = Offset(x, y);
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
      final dragged = (x - _scaleOrigin.dx) * _scaleRatio;
    // Every 100 panel pixels to the right doubles the size and every 100 to
    // the left halves it, so it shrinks as easily as it grows.
      final factor = math.max(
        0.01,
      math.pow(2, dragged / 100).toDouble(),
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
    final factor = math.max(0.01, param / _gizmoStartParam);
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
    _studio.setActivePosition((
      sx + dx * along,
      sy + dy * along,
      sz + dz * along,
    ));
  }

void _endDrag() {
    _surfaceDrag = false;
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
        _gizmoHighlight.value = _viewport.gizmoAxisAt(
          pointer,
          position,
          scale,
          kGizmoLength,
        );
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
          onPointerMove: (event) => _handleMiddleDrag(event, scale),
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
                    painter: FrameOverlayPainter(
                      viewport: _viewport,
                      renderWidth: _renderSize.width,
                      repaint: _overlayTick,
                    ),
                  ),
                ),
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
              _studio.addShape(
                details.data,
                local.dx / scale,
                local.dy / scale,
              ),
            );
          },
          builder: (context, candidate, rejected) => render,
        );
      },
    );
  }

  /// What the viewport shows when the graphics could not be started: the
  /// reason, in plain words. Menus, panels and saving still work.
  Widget _graphicsErrorView(String message) {
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(
                Icons.warning_amber_rounded,
                color: HuskColors.orange,
                size: 36,
              ),
              const SizedBox(height: 12),
              const Text(
                'The viewport could not start',
                style: TextStyle(fontSize: 16, fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 8),
              Text(
                message,
                textAlign: TextAlign.center,
                style: const TextStyle(
                  color: HuskColors.textSecondary,
                  fontSize: 13,
                ),
              ),
              const SizedBox(height: 8),
              const Text(
                'The rest of Husk still works. Restart Husk after updating '
                'your graphics driver.',
                textAlign: TextAlign.center,
                style: TextStyle(
                  color: HuskColors.textSecondary,
                  fontSize: 13,
                ),
              ),
            ],
          ),
        ),
      ),
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
            child: _graphicsError != null
                ? _graphicsErrorView(_graphicsError!)
                : textureId == null
                ? const Center(
                    child: CircularProgressIndicator(color: HuskColors.orange),
                  )
                : Stack(
                    fit: StackFit.expand,
                    children: [
                      _buildRender(context, textureId),
                      if (_studio.viewportMaximized)
                        Positioned(
                          left: 12,
                          top: 12,
                          child: ToolColumn(studio: _studio, floating: true),
                        ),
                      if (_studio.viewportMaximized)
                        Positioned(
                          left: 0,
                          right: 0,
                          bottom: 12,
                          child: Center(
                            child: MaximizedBar(actions: widget.actions),
                          ),
                        ),
                    ],
                  ),
          ),
        ),
      ],
    );
  }
}
