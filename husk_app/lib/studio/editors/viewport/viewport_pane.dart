import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/studio/editors/viewport/skeleton_overlay.dart';
import 'package:husk_app/studio/editors/viewport/viewport_tool_strip.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/messages.dart';

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

  // Ticks ~30 times a second so the skeleton overlay repaints as the
  // camera orbits or bones move. Only the overlay listens to it.
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
    }
  }

  void _handlePanStart(DragStartDetails details, double scale) {
    _draggedJoint = null;
    _dragMovesCharacter = false;
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
    }
  }

  void _handlePanUpdate(DragUpdateDetails details, double scale) {
    final x = details.localPosition.dx / scale;
    final y = details.localPosition.dy / scale;
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

  Widget _buildRender(BuildContext context, int textureId) {
    return LayoutBuilder(
      builder: (context, constraints) {
        _requestSize(
          constraints.biggest,
          MediaQuery.devicePixelRatioOf(context),
        );
        // From render pixels (what the engine reports) to panel pixels.
        final scale = constraints.maxWidth / _renderSize.width;
        return Listener(
          onPointerSignal: _handlePointerSignal,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapUp: (details) => _handleTap(details, scale),
            onPanStart: (details) => _handlePanStart(details, scale),
            onPanUpdate: (details) => _handlePanUpdate(details, scale),
            onPanEnd: (_) => _draggedJoint = null,
            onPanCancel: () => _draggedJoint = null,
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
              ],
            ),
          ),
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
