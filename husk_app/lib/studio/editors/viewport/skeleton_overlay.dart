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
  bool shouldRepaint(covariant SkeletonOverlayPainter oldDelegate) => true;
}
