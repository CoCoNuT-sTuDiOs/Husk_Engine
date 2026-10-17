import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// How long a gizmo arrow is drawn, in panel pixels, whatever the camera
/// distance.
const double kGizmoLength = 90;

/// Draws the Move gizmo over the render: three arrows (X red, Y green,
/// Z blue) from the active object's origin. [highlighted] is the arrow to
/// light up (0 = X, 1 = Y, 2 = Z), or null.
class GizmoOverlayPainter extends CustomPainter {
  GizmoOverlayPainter({
    required this.viewport,
    required this.studio,
    required this.renderWidth,
    required this.highlighted,
    required Listenable repaint,
  }) : super(repaint: repaint);

  final ViewportController viewport;
  final StudioController studio;
  final double renderWidth;
  final ValueListenable<int?> highlighted;

  static const List<Color> _axisColors = [
    HuskColors.error,
    HuskColors.success,
    HuskColors.blue,
  ];

  @override
  void paint(Canvas canvas, Size size) {
    if (studio.tool != StudioTool.moveObject) return;
    final scale = size.width / renderWidth;
    final axes = viewport.gizmoAxes(
      studio.activeTransform.position,
      kGizmoLength / scale,
    );
    if (axes.isEmpty) return;
    final lit = highlighted.value;

    for (final axis in axes) {
      final start = axis.start * scale;
      final tip = axis.tip * scale;
      final along = tip - start;
      // An arrow pointing straight at the camera has nothing to draw.
      if (along.distance < 4) continue;

      final isLit = axis.index == lit;
      final base = _axisColors[axis.index];
      final color = isLit ? Color.lerp(base, Colors.white, 0.45)! : base;
      final direction = along / along.distance;
      final sideways = Offset(-direction.dy, direction.dx);
      final headBase = tip - direction * 16;

      canvas.drawLine(
        start,
        headBase,
        Paint()
          ..color = color
          ..strokeWidth = isLit ? 4 : 3
          ..strokeCap = StrokeCap.round,
      );
      final head = Path()
        ..moveTo(tip.dx, tip.dy)
        ..lineTo(headBase.dx + sideways.dx * 6, headBase.dy + sideways.dy * 6)
        ..lineTo(headBase.dx - sideways.dx * 6, headBase.dy - sideways.dy * 6)
        ..close();
      canvas.drawPath(head, Paint()..color = color);
    }

    final origin = axes.first.start * scale;
    canvas.drawCircle(origin, 5, Paint()..color = HuskColors.textPrimary);
    canvas.drawCircle(
      origin,
      5,
      Paint()
        ..color = HuskColors.canvas
        ..strokeWidth = 1.5
        ..style = PaintingStyle.stroke,
    );
  }

  @override
  bool shouldRepaint(covariant GizmoOverlayPainter oldDelegate) => true;
}