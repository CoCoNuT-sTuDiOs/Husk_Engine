import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// While looking through the scene camera, darkens everything outside the
/// output frame and outlines the frame, so you see exactly what the video
/// will show.
class FrameOverlayPainter extends CustomPainter {
  FrameOverlayPainter({
    required this.viewport,
    required this.renderWidth,
    required Listenable repaint,
  }) : super(repaint: repaint);

  final ViewportController viewport;

  // The frame arrives from the engine in render pixels; the panel is shown
  // at a different size, so everything is scaled by this ratio.
  final double renderWidth;

  @override
  void paint(Canvas canvas, Size size) {
    final frame = viewport.cameraFrame();
    if (frame == null) return;
    final scale = size.width / renderWidth;
    final rect = Rect.fromLTRB(
      frame.left * scale,
      frame.top * scale,
      frame.right * scale,
      frame.bottom * scale,
    );
    final outside = Path()
      ..addRect(Offset.zero & size)
      ..addRect(rect)
      ..fillType = PathFillType.evenOdd;
    canvas.drawPath(outside, Paint()..color = const Color(0xB3000000));
    canvas.drawRect(
      rect,
      Paint()
        ..color = HuskColors.textSecondary
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1,
    );
  }

  @override
  bool shouldRepaint(covariant FrameOverlayPainter oldDelegate) => true;
}