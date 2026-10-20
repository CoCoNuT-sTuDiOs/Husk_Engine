import 'dart:ui';

import 'package:flutter/services.dart';
import 'package:husk_app/src/rust/api/simple.dart' as rust;

/// One gizmo arrow as drawn on screen: which axis it is (0 = X, 1 = Y,
/// 2 = Z) and its two ends, in render pixels.
class GizmoAxis {
  const GizmoAxis(this.index, this.start, this.tip);

  final int index;
  final Offset start;
  final Offset tip;
}

/// One rotation ring as drawn on screen: which axis it turns about (0 = X,
/// 1 = Y, 2 = Z) and its points, in render pixels, closing the loop.
class GizmoRing {
  const GizmoRing(this.index, this.points);

  final int index;
  final List<Offset> points;
}

class ViewportController {
  static const MethodChannel _channel = MethodChannel('husk/texture');

  /// The id of the texture the engine renders into.
  Future<int> textureId() async {
    final int id = await _channel.invokeMethod('getTextureId');
    return id;
  }

  /// Changes the size the engine renders at (in pixels).
  Future<void> resize(int width, int height) async {
    await _channel.invokeMethod('resize', {'width': width, 'height': height});
  }

  void orbit(double deltaYaw, double deltaPitch) {
    rust.orbitCamera(deltaYaw: deltaYaw, deltaPitch: deltaPitch);
  }

  /// A factor above 1 moves the camera away, below 1 closer.
  void zoom(double factor) {
    rust.zoomCamera(factor: factor);
  }

  /// Screen position (in render pixels) of every joint; null for a joint
  /// behind the camera.
  List<(double, double)?> jointPositions() => rust.jointScreenPositions();

  /// The parent of every bone; null for a root.
  List<BigInt?> boneParents() => rust.boneParents();

  /// True when [joint] has no parent, so dragging it moves the whole
  /// character.
  bool isRoot(BigInt joint) => boneParents()[joint.toInt()] == null;

  /// The joint nearest to [tap] (in panel pixels) if one is close enough.
  /// [scale] converts render pixels to panel pixels.
  BigInt? jointAt(Offset tap, double scale) {
    const hitRadius = 14.0;
    final positions = jointPositions();
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

  /// Drags a joint to render-pixel position (x, y): swings its parent, or
  /// bends the limb above it when [ik] is on.
  void dragJoint(BigInt joint, double x, double y, {required bool ik}) {
    if (ik) {
      rust.dragLimb(boneIndex: joint, x: x, y: y);
    } else {
      rust.dragBone(boneIndex: joint, x: x, y: y);
    }
  }

  /// Moves the whole character so its root follows (x, y).
  void moveCharacter(double x, double y) {
    rust.moveCharacter(x: x, y: y);
  }

  /// While looking through the scene camera: where the output frame sits in
  /// the render, in render pixels. Null otherwise.
  Rect? cameraFrame() {
    final frame = rust.cameraFrameRect();
    if (frame == null) return null;
    final (x, y, width, height) = frame;
    return Rect.fromLTWH(x, y, width, height);
  }

  /// The three gizmo directions: X, Y and Z in the scene.
  static const List<(double, double, double)> gizmoAxisDirections = [
    (1.0, 0.0, 0.0),
    (0.0, 1.0, 0.0),
    (0.0, 0.0, 1.0),
  ];

  /// The gizmo arrows for an object at [position], each [pixels] long on
  /// screen, in render pixels. An arrow is left out when the object is
  /// behind the camera.
  List<GizmoAxis> gizmoAxes(
    (double, double, double) position,
    double pixels, {
    List<(double, double, double)> directions = gizmoAxisDirections,
  }) {
    final axes = <GizmoAxis>[];
    for (var i = 0; i < directions.length; i++) {
      final ends = rust.gizmoAxisEnds(
        origin: position,
        axis: directions[i],
        pixels: pixels,
      );

      if (ends == null) continue;
      final (start, tip) = ends;
      axes.add(GizmoAxis(i, Offset(start.$1, start.$2), Offset(tip.$1, tip.$2)));
    }
    return axes;
  }

  /// The gizmo arrow under [pointer] (panel pixels), if any: 0 = X, 1 = Y,
  /// 2 = Z. [scale] converts render pixels to panel pixels, and
  /// [length] is how long the arrows are drawn in panel pixels. An arrow
  /// that points straight at the camera has no length and cannot be hit.
  int? gizmoAxisAt(
    Offset pointer,
    (double, double, double) position,
    double scale,
    double length, {
    List<(double, double, double)> directions = gizmoAxisDirections,
  }) {
    const hitRadius = 12.0;
    int? best;
    var bestDistance = hitRadius;
    for (final axis
        in gizmoAxes(position, length / scale, directions: directions)) {

      final start = axis.start * scale;
      final tip = axis.tip * scale;
      if ((tip - start).distance < 4) continue;
      final distance = _distanceToSegment(pointer, start, tip);
      if (distance <= bestDistance) {
        best = axis.index;
        bestDistance = distance;
      }
    }
    return best;
  }

  /// How far along gizmo axis [axis] (0 = X, 1 = Y, 2 = Z), measured from
  /// [position], the cursor at render pixel (x, y) falls. Null when the
  /// cursor ray runs parallel to the axis.
  double? axisParam(
    (double, double, double) position,
    int axis,
    double x,
    double y, {
    List<(double, double, double)> directions = gizmoAxisDirections,
  }) =>
      rust.axisDragParam(
        origin: position,
        axis: directions[axis],
        x: x,
        y: y,
      );

  /// The centre handle of the scale gizmo, after the three axis handles.
  static const int uniformScaleHandle = 3;

  /// Where [position] is on screen, in render pixels; null when it is
  /// behind the camera.
  Offset? screenPoint((double, double, double) position) {
    final axes = gizmoAxes(position, 10);
    return axes.isEmpty ? null : axes.first.start;
  }

  /// The scale handle under [pointer] (panel pixels), if any: 0 = X, 1 = Y,
  /// 2 = Z along the object's own axes [directions], or
  /// [uniformScaleHandle] for the centre box, which wins where it overlaps
  /// an arrow. [scale] converts render pixels to panel pixels, and [length]
  /// is how long the arrows are drawn, in panel pixels.
  int? scaleHandleAt(
    Offset pointer,
    (double, double, double) position,
    double scale,
    double length,
    List<(double, double, double)> directions,
  ) {
    final origin = screenPoint(position);
    if (origin == null) return null;
    if ((pointer - origin * scale).distance <= 12) return uniformScaleHandle;
    return gizmoAxisAt(
      pointer,
      position,
      scale,
      length,
      directions: directions,
    );
  }

  /// How many segments a rotation ring is drawn with.
  static const int _ringSegments = 64;

  /// The three rotation rings for an object at [position], each [radius]
  /// pixels in radius on screen, in render pixels. A ring is left out when
  /// the object is behind the camera.
  List<GizmoRing> gizmoRings((double, double, double) position, double radius) {
    final rings = <GizmoRing>[];
    for (var i = 0; i < gizmoAxisDirections.length; i++) {
      final points = rust.gizmoRing(
        origin: position,
        axis: gizmoAxisDirections[i],
        pixels: radius,
        segments: _ringSegments,
      );
      if (points == null) continue;
      rings.add(GizmoRing(i, [for (final p in points) Offset(p.$1, p.$2)]));
    }
    return rings;
  }

  /// The rotation ring under [pointer] (panel pixels), if any: 0 = X,
  /// 1 = Y, 2 = Z. [scale] converts render pixels to panel pixels, and
  /// [radius] is how big the rings are drawn, in panel pixels.
  int? gizmoRingAt(
    Offset pointer,
    (double, double, double) position,
    double scale,
    double radius,
  ) {
    const hitRadius = 10.0;
    int? best;
    var bestDistance = hitRadius;
    for (final ring in gizmoRings(position, radius / scale)) {
      for (var i = 0; i + 1 < ring.points.length; i++) {
        final distance = _distanceToSegment(
          pointer,
          ring.points[i] * scale,
          ring.points[i + 1] * scale,
        );
        if (distance <= bestDistance) {
          best = ring.index;
          bestDistance = distance;
        }
      }
    }
    return best;
  }

  /// The angle (radians) of the cursor at render pixel (x, y) around ring
  /// axis [axis] (0 = X, 1 = Y, 2 = Z) through [position]. Null when that
  /// ring is seen almost edge-on.
  double? ringAngle(
    (double, double, double) position,
    int axis,
    double x,
    double y,
  ) =>
      rust.ringDragAngle(
        origin: position,
        axis: gizmoAxisDirections[axis],
        x: x,
        y: y,
      );

  /// The smallest signed turn (radians) that takes angle [from] to [to].
  double angleDifference(double from, double to) =>
      rust.angleDifference(from: from, to: to);

  /// The rotation (Euler degrees) of an object that had rotation [start]
  /// and has since been turned by [angle] radians about scene axis [axis].
  (double, double, double) rotatedEuler(
    (double, double, double) start,
    int axis,
    double angle,
  ) =>
      rust.rotatedEulerDegrees(
        startDegrees: start,
        axis: gizmoAxisDirections[axis],
        angle: angle,
      );

  static double _distanceToSegment(Offset point, Offset a, Offset b) {
    final ab = b - a;
    final lengthSquared = ab.dx * ab.dx + ab.dy * ab.dy;
    if (lengthSquared == 0) return (point - a).distance;
    final along =
        ((point.dx - a.dx) * ab.dx + (point.dy - a.dy) * ab.dy) / lengthSquared;
    final t = along.clamp(0.0, 1.0).toDouble();
    return (point - (a + ab * t)).distance;
  }
}