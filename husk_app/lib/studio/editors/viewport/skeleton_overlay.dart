import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// Draws the skeleton (joint dots and the lines between them) over the
/// render.
class SkeletonOverlayPainter extends CustomPainter {
  SkeletonOverlayPainter({
    required this.viewport,
    required this.studio,
    required this.renderWidth,
    required Listenable repaint,
  }) : super(repaint: repaint);

  final ViewportController viewport;
  final StudioController studio;

  // Joint positions arrive from the engine in render pixels; the panel is
  // shown at a different size, so everything is scaled by this ratio.
  final double renderWidth;

  @override
  void paint(Canvas canvas, Size size) {
    final positions = viewport.jointPositions();
    if (positions.isEmpty) return;
    final parents = viewport.boneParents();
    final scale = size.width / renderWidth;
    final selectedIndex = studio.selectedBone?.toInt();

    final linePaint = Paint()
      ..color = HuskColors.blue
      ..strokeWidth = 2
      ..style = PaintingStyle.stroke;
      final dotFill = Paint()..color = HuskColors.orange;
    final boneFill = Paint()..color = HuskColors.blue.withValues(alpha: 0.35);
    final selectedBoneFill = Paint()
      ..color = HuskColors.blue.withValues(alpha: 0.7);
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
      final selected = i == selectedIndex;
      _drawBone(
        canvas,
        toCanvas(parentJoint),
        toCanvas(joint),
        selected ? selectedBoneFill : boneFill,
        selected ? selectedRing : linePaint,
      );
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
  bool shouldRepaint(covariant SkeletonOverlayPainter oldDelegate) => true;

  /// One bone as Blender draws it by default (octahedral): a long diamond from
  /// the [head] to the [tail], widest 10 percent of the way out from the head,
  /// where it is 10 percent of the bone's length to each side. A bone too short
  /// on screen to show a shape is a plain line.
  void _drawBone(
    Canvas canvas,
    Offset head,
    Offset tail,
    Paint fill,
    Paint outline,
  ) {
    final along = tail - head;
    final length = along.distance;
    if (length < 4) {
      canvas.drawLine(head, tail, outline);
      return;
    }
    final direction = along / length;
    final side = Offset(-direction.dy, direction.dx);
    final waist = head + along * 0.1;
    final half = length * 0.1;
    final path = Path()
      ..moveTo(head.dx, head.dy)
      ..lineTo(waist.dx + side.dx * half, waist.dy + side.dy * half)
      ..lineTo(tail.dx, tail.dy)
      ..lineTo(waist.dx - side.dx * half, waist.dy - side.dy * half)
      ..close();
    canvas.drawPath(path, fill);
    canvas.drawPath(path, outline);
  }
}
