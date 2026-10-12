import 'dart:ui';

import 'package:flutter/services.dart';
import 'package:husk_app/src/rust/api/simple.dart' as rust;

/// Everything the viewport asks of the engine: the render texture and its
/// size, the camera, joint hit-testing and joint drags. The viewport widget
/// calls these and never talks to Rust itself.
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
}
