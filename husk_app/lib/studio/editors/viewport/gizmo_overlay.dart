import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// How long a gizmo arrow is drawn, in panel pixels, whatever the camera
/// distance.
const double kGizmoLength = 90;

/// How big a rotation ring is drawn (its radius), in panel pixels.
const double kGizmoRingRadius = 80;

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
    switch (studio.tool) {
      case StudioTool.moveObject:
        _paintArrows(canvas, size);
      case StudioTool.rotateObject:
        _paintRings(canvas, size);
      case StudioTool.scaleObject:
        _paintScaleHandles(canvas, size);
      case StudioTool.select:
      case StudioTool.placeBones:
      case StudioTool.move:
        break;
    }
  }

  void _paintArrows(Canvas canvas, Size size) {
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

  void _paintScaleHandles(Canvas canvas, Size size) {
    final scale = size.width / renderWidth;
    final axes = viewport.gizmoAxes(
      studio.activeTransform.position,
      kGizmoLength / scale,
      directions: studio.activeLocalAxes,
    );
    if (axes.isEmpty) return;
    final lit = highlighted.value;

    for (final axis in axes) {
      final start = axis.start * scale;
      final tip = axis.tip * scale;
      // An arrow pointing straight at the camera has nothing to draw.
      if ((tip - start).distance < 4) continue;

      final isLit = axis.index == lit;
      final base = _axisColors[axis.index];
      final color = isLit ? Color.lerp(base, Colors.white, 0.45)! : base;
      canvas.drawLine(
        start,
        tip,
        Paint()
          ..color = color
          ..strokeWidth = isLit ? 4 : 3
          ..strokeCap = StrokeCap.round,
      );
      // A square at the end of the line marks it as a scale handle.
      canvas.drawRect(
        Rect.fromCenter(center: tip, width: 14, height: 14),
        Paint()..color = color,
      );
    }

    // The centre box scales all three axes together.
    final centre = axes.first.start * scale;
    final centreLit = lit == ViewportController.uniformScaleHandle;
    final box = Rect.fromCenter(center: centre, width: 18, height: 18);
    canvas.drawRect(
      box,
      Paint()..color = centreLit ? Colors.white : HuskColors.textPrimary,
    );
    canvas.drawRect(
      box,
      Paint()
        ..color = HuskColors.canvas
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5,
    );
  }

  void _paintRings(Canvas canvas, Size size) {
    final scale = size.width / renderWidth;
    final rings = viewport.gizmoRings(
      studio.activeTransform.position,
      kGizmoRingRadius / scale,
    );
    final lit = highlighted.value;

    for (final ring in rings) {
      final isLit = ring.index == lit;
      final base = _axisColors[ring.index];
      final color = isLit ? Color.lerp(base, Colors.white, 0.45)! : base;
      final path = Path();
      for (var i = 0; i < ring.points.length; i++) {
        final point = ring.points[i] * scale;
        if (i == 0) {
          path.moveTo(point.dx, point.dy);
        } else {
          path.lineTo(point.dx, point.dy);
        }
      }
      canvas.drawPath(
        path,
        Paint()
          ..color = color
          ..style = PaintingStyle.stroke
          ..strokeWidth = isLit ? 4 : 2.5
          ..strokeCap = StrokeCap.round
          ..strokeJoin = StrokeJoin.round,
      );
    }
  }

  @override
  bool shouldRepaint(covariant GizmoOverlayPainter oldDelegate) => true;

}