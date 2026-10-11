import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/src/rust/api/simple.dart';
import 'package:husk_app/studio/studio_controller.dart';
import 'package:husk_app/studio/studio_theme.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

const MethodChannel _huskChannel = MethodChannel('husk/texture');

// The renderer never draws more than this many pixels on its long side.
const double _maxRenderSide = 1920;

/// The live render, filling its whole panel. The engine renders at the
/// panel's size, so nothing is stretched. Dragging orbits the camera, the
/// mouse wheel and the - and = keys zoom, tapping a joint dot selects it,
/// and the corner button (or Ctrl+Space) maximizes the viewport.
class ViewportPane extends StatefulWidget {
  const ViewportPane({super.key, required this.controller});

  final StudioController controller;

  @override
  State<ViewportPane> createState() => _ViewportPaneState();
}

class _ViewportPaneState extends State<ViewportPane> {
  int? _textureId;

  // The size the engine is rendering at right now, in pixels. The native
  // side starts at 1000 x 1000 until the first resize.
  Size _renderSize = const Size(1000, 1000);
  Size? _requestedSize;
  Timer? _resizeTimer;
  bool _firstResizeDone = false;

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
    final int id = await _huskChannel.invokeMethod('getTextureId');
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
      await _huskChannel.invokeMethod('resize', {
        'width': size.width.toInt(),
        'height': size.height.toInt(),
      });
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
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.space &&
        HardwareKeyboard.instance.isControlPressed) {
      widget.controller.toggleViewportMaximized();
      return true;
    }
    if (event is KeyDownEvent || event is KeyRepeatEvent) {
      final key = event.logicalKey;
      if (key == LogicalKeyboardKey.minus ||
          key == LogicalKeyboardKey.numpadSubtract) {
        zoomCamera(factor: 1.08); // zoom out
        return true;
      }
      if (key == LogicalKeyboardKey.equal ||
          key == LogicalKeyboardKey.add ||
          key == LogicalKeyboardKey.numpadAdd) {
        zoomCamera(factor: 1 / 1.08); // zoom in
        return true;
      }
    }
    return false;
  }

  void _handlePointerSignal(PointerSignalEvent event) {
    if (event is PointerScrollEvent) {
      // Scrolling down moves the camera away, scrolling up brings it closer.
      zoomCamera(factor: event.scrollDelta.dy > 0 ? 1.08 : 1 / 1.08);
    }
  }

  void _handleOrbitDrag(DragUpdateDetails details) {
    // Degrees-per-pixel-dragged, converted to radians for the Rust side.
    const dragSensitivity = 0.01;
    orbitCamera(
      deltaYaw: details.delta.dx * dragSensitivity,
      deltaPitch: -details.delta.dy * dragSensitivity,
    );
  }

  void _handleTap(TapUpDetails details, double scale) {
    // Tapping a joint dot selects it; tapping empty space deselects.
    widget.controller.selectBone(_jointAt(details.localPosition, scale));
  }

  BigInt? _jointAt(Offset tap, double scale) {
    const hitRadius = 14.0;
    final positions = jointScreenPositions();
    int? best;
    var bestDistance = hitRadius;
    for (var i = 0; i < positions.length; i++) {
      final joint = positions[i];
      if (joint == null) continue;
      final distance =
          (Offset(joint.$1 * scale, joint.$2 * scale) - tap).distance;
      if (distance <= bestDistance) {
        best = i;
        bestDistance = distance;
      }
    }
    return best == null ? null : BigInt.from(best);
  }

  @override
  Widget build(BuildContext context) {
    final textureId = _textureId;
    return Container(
      color: HuskColors.canvas,
      child: textureId == null
          ? const Center(
              child: CircularProgressIndicator(color: HuskColors.orange),
            )
          : LayoutBuilder(
              builder: (context, constraints) {
                _requestSize(
                  constraints.biggest,
                  MediaQuery.devicePixelRatioOf(context),
                );
                // From render pixels (what Rust reports) to panel pixels.
                final scale = constraints.maxWidth / _renderSize.width;
                return Stack(
                  children: [
                    Positioned.fill(
                      child: Listener(
                        onPointerSignal: _handlePointerSignal,
                        child: GestureDetector(
                          behavior: HitTestBehavior.opaque,
                          onTapUp: (details) => _handleTap(details, scale),
                          onPanUpdate: _handleOrbitDrag,
                          child: Stack(
                            fit: StackFit.expand,
                            children: [
                              Texture(textureId: textureId),
                              IgnorePointer(
                                child: CustomPaint(
                                  painter: _SkeletonOverlayPainter(
                                    renderWidth: _renderSize.width,
                                    controller: widget.controller,
                                    repaint: Listenable.merge([
                                      _overlayTick,
                                      widget.controller,
                                    ]),
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                    Positioned(
                      top: 8,
                      right: 8,
                      child: _MaximizeButton(controller: widget.controller),
                    ),
                  ],
                );
              },
            ),
    );
  }
}

class _MaximizeButton extends StatelessWidget {
  const _MaximizeButton({required this.controller});

  final StudioController controller;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final maximized = controller.viewportMaximized;
        return Tooltip(
          message: maximized
              ? 'Restore panels (Ctrl+Space)'
              : 'Maximize viewport (Ctrl+Space)',
          child: Material(
            color: HuskColors.panel.withValues(alpha: 0.85),
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(6),
              side: const BorderSide(color: HuskColors.border),
            ),
            child: InkWell(
              borderRadius: BorderRadius.circular(6),
              onTap: controller.toggleViewportMaximized,
              child: SizedBox(
                width: 32,
                height: 32,
                child: Center(
                  child: PhosphorIcon(
                    maximized
                        ? PhosphorIconsRegular.arrowsIn
                        : PhosphorIconsRegular.arrowsOut,
                    size: 18,
                    color: HuskColors.textPrimary,
                  ),
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}

class _SkeletonOverlayPainter extends CustomPainter {
  _SkeletonOverlayPainter({
    required this.renderWidth,
    required this.controller,
    required Listenable repaint,
  }) : super(repaint: repaint);

  // Joint positions arrive from Rust in render pixels; the panel is shown at
  // a different size, so everything is scaled by this ratio.
  final double renderWidth;
  final StudioController controller;

  @override
  void paint(Canvas canvas, Size size) {
    final positions = jointScreenPositions();
    if (positions.isEmpty) return;
    final parents = boneParents();
    final scale = size.width / renderWidth;
    final selectedIndex = controller.selectedBone?.toInt();

    final linePaint = Paint()
      ..color = HuskColors.blue
      ..strokeWidth = 2
      ..style = PaintingStyle.stroke;
    final dotFill = Paint()..color = HuskColors.orange;
    final rootFill = Paint()..color = HuskColors.success;
    final dotOutline = Paint()
      ..color = HuskColors.canvas
      ..strokeWidth = 1.5
      ..style = PaintingStyle.stroke;
    final selectedRing = Paint()
      ..color = HuskColors.textPrimary
      ..strokeWidth = 2.5
      ..style = PaintingStyle.stroke;

    Offset toCanvas((double, double) p) => Offset(p.$1 * scale, p.$2 * scale);

    for (var i = 0; i < positions.length && i < parents.length; i++) {
      final parent = parents[i];
      final joint = positions[i];
      if (parent == null || joint == null) continue;
      final parentIndex = parent.toInt();
      if (parentIndex >= positions.length) continue;
      final parentJoint = positions[parentIndex];
      if (parentJoint == null) continue;
      canvas.drawLine(toCanvas(parentJoint), toCanvas(joint), linePaint);
    }

    for (var i = 0; i < positions.length; i++) {
      final joint = positions[i];
      if (joint == null) continue;
      final center = toCanvas(joint);
      final isRoot = i < parents.length && parents[i] == null;
      final radius = isRoot ? 9.0 : 6.0;
      canvas.drawCircle(center, radius, isRoot ? rootFill : dotFill);
      canvas.drawCircle(center, radius, dotOutline);
      if (i == selectedIndex) {
        canvas.drawCircle(center, 11, selectedRing);
      }
    }
  }

  @override
  bool shouldRepaint(covariant _SkeletonOverlayPainter oldDelegate) => true;
}