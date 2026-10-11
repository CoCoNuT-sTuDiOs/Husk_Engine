import 'dart:async';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/src/rust/api/simple.dart';
import 'package:husk_app/studio/studio_controller.dart';
import 'package:husk_app/studio/studio_theme.dart';

const MethodChannel _huskChannel = MethodChannel('husk/texture');

/// The live render, with the skeleton drawn over it. Dragging orbits the
/// camera, the mouse wheel and the - and = keys zoom, and tapping a joint
/// dot selects it.
class ViewportPane extends StatefulWidget {
  const ViewportPane({super.key, required this.controller});

  final StudioController controller;

  @override
  State<ViewportPane> createState() => _ViewportPaneState();
}

class _ViewportPaneState extends State<ViewportPane> {
  // Must match the renderer's resolution set in flutter_window.cpp.
  static const double _renderSize = 1000;

  int? _textureId;

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

  bool _handleKey(KeyEvent event) {
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
      alignment: Alignment.center,
      child: textureId == null
          ? const CircularProgressIndicator(color: HuskColors.orange)
          : LayoutBuilder(
              builder: (context, constraints) {
                // The render is square, so show it at the largest square
                // that fits the pane.
                final side = constraints.biggest.shortestSide;
                final scale = side / _renderSize;
                return SizedBox(
                  width: side,
                  height: side,
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
                                renderSize: _renderSize,
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
                );
              },
            ),
    );
  }
}

class _SkeletonOverlayPainter extends CustomPainter {
  _SkeletonOverlayPainter({
    required this.renderSize,
    required this.controller,
    required Listenable repaint,
  }) : super(repaint: repaint);

  // Joint positions arrive from Rust in render pixels; the canvas may be
  // displayed at a different size, so everything is scaled by this ratio.
  final double renderSize;
  final StudioController controller;

  @override
  void paint(Canvas canvas, Size size) {
    final positions = jointScreenPositions();
    if (positions.isEmpty) return;
    final parents = boneParents();
    final scale = size.width / renderSize;
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