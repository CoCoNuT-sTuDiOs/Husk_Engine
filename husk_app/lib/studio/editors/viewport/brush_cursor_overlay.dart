import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';

/// The circle that follows the cursor in Weight Paint mode and shows how far
/// one touch of the brush reaches. Its radius is in panel pixels, the same
/// unit as the Radius slider.
class BrushCursorPainter extends CustomPainter {
  BrushCursorPainter({required this.studio, required this.cursor})
    : super(repaint: Listenable.merge([studio, cursor]));

  final StudioController studio;
  final ValueNotifier<Offset?> cursor;

  @override
  void paint(Canvas canvas, Size size) {
    final centre = cursor.value;
    if (centre == null || studio.mode != 'weight_paint') return;
    final radius = studio.brushRadius;
    canvas.drawCircle(
      centre,
      radius,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 3
        ..color = const Color(0x99000000),
    );
    canvas.drawCircle(
      centre,
      radius,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5
        ..color = const Color(0xFFFFFFFF),
    );
  }

  @override
  bool shouldRepaint(BrushCursorPainter oldDelegate) =>
      oldDelegate.studio != studio || oldDelegate.cursor != cursor;
}