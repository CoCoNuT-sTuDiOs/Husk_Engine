import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/src/rust/api/simple.dart';
import 'package:husk_app/src/rust/frb_generated.dart';

const MethodChannel _huskChannel = MethodChannel('husk/texture');

Future<void> main() async {
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      debugShowCheckedModeBanner: false,
      home: Scaffold(body: HuskTextureView()),
    );
  }
}

class HuskTextureView extends StatefulWidget {
  const HuskTextureView({super.key});

  @override
  State<HuskTextureView> createState() => _HuskTextureViewState();
}

class _HuskTextureViewState extends State<HuskTextureView> {
  // Must match the renderer's resolution set in flutter_window.cpp.
  static const double _renderSize = 1000;

  int? _textureId;
  BigInt? _lastBoneIndex;
  double _rotationAngle = 0.0;
  BigInt? _draggedJoint;
  bool _ikMode = false;
  
  // Ticks ~30 times a second so the skeleton overlay repaints as the
  // camera orbits or bones move. Only the overlay listens to it, so the
  // rest of the widget tree isn't rebuilt every tick.
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
    setState(() {
      _textureId = id;
    });
  }

  void _handleTap(TapUpDetails details, double scale) {
    // Tapping a joint dot selects it instead of placing a new bone.
    final hitJoint = _jointAt(details.localPosition, scale);
    if (hitJoint != null) {
      setState(() {
        _lastBoneIndex = hitJoint;
      });
      debugPrint('Selected joint $hitJoint');
      return;
    }

    final result = placeBone(
      x: details.localPosition.dx / scale,
      y: details.localPosition.dy / scale,
      parent: _lastBoneIndex,
    );

    if (result != null) {
      final (index, x, y, z) = result;
      setState(() {
        _lastBoneIndex = index;
      });
      debugPrint(
        'Placed bone $index at (${x.toStringAsFixed(3)}, '
        '${y.toStringAsFixed(3)}, ${z.toStringAsFixed(3)}) '
        '— total bones: ${boneCount()}',
      );
    } else {
      debugPrint('Bone placement: click missed the mesh');
    }
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

  void _handleComputeWeights() {
    final timer = Stopwatch()..start();
    final success = computeWeights();
    timer.stop();
    debugPrint(
      success
          ? 'Weights computed for ${boneCount()} bones '
              'in ${timer.elapsedMilliseconds} ms'
          : 'Weight computation failed (no bones placed yet?)',
    );
  }
  void _handleRotationChanged(double value) {
    setState(() {
      _rotationAngle = value;
    });
    // Temporary: drives the root bone so the whole chain (and its overlay)
    // visibly swings; drag-posing in a later step replaces this slider.
    if (_lastBoneIndex != null) {
      rotateBone(boneIndex: BigInt.zero, angleDegrees: value);
    }
  }

  bool _handleKey(KeyEvent event) {
    if ((event is KeyDownEvent || event is KeyRepeatEvent) &&
        event.logicalKey == LogicalKeyboardKey.keyZ &&
        HardwareKeyboard.instance.isControlPressed) {
      _handleUndo();
      return true;
    }
    return false;
  }

  void _handleUndo() {
    final (removed, parent) = undoLastBone();
    if (!removed) {
      debugPrint('Nothing to undo');
      return;
    }
    setState(() {
      _lastBoneIndex = parent;
    });
    debugPrint(
      'Undid the last bone; selected joint: $parent, total bones: ${boneCount()}',
    );
  }

  void _handlePanStart(DragStartDetails details, double scale) {
    // A pan that begins on a joint dot drags that joint; anywhere else
    // it orbits the camera as before.
    _draggedJoint = _jointAt(details.localPosition, scale);
  }

  void _handlePanUpdate(DragUpdateDetails details, double scale) {
    final joint = _draggedJoint;
    if (joint == null) {
      _handleOrbitDrag(details);
      return;
    }
        final x = details.localPosition.dx / scale;
    final y = details.localPosition.dy / scale;
    if (_ikMode) {
      dragLimb(boneIndex: joint, x: x, y: y);
    } else {
      dragBone(boneIndex: joint, x: x, y: y);
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

  @override
  Widget build(BuildContext context) {
    if (_textureId == null) {
      return const Center(child: CircularProgressIndicator());
    }
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Flexible(
            child: AspectRatio(
              aspectRatio: 1,
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final scale = constraints.maxWidth / _renderSize;
                  return GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onTapUp: (details) => _handleTap(details, scale),
                    onPanStart: (details) => _handlePanStart(details, scale),
                    onPanUpdate: (details) => _handlePanUpdate(details, scale),
                    onPanEnd: (_) => _draggedJoint = null,
                    onPanCancel: () => _draggedJoint = null,                    
                    child: Stack(
                      fit: StackFit.expand,
                      children: [
                        Texture(textureId: _textureId!),
                        IgnorePointer(
                          child: CustomPaint(
                            painter: _SkeletonOverlayPainter(
                              renderSize: _renderSize,
                              selectedIndex: _lastBoneIndex?.toInt(),
                              repaint: _overlayTick,
                            ),
                          ),
                        ),
                      ],
                    ),
                  );
                },
              ),
            ),
          ),
          const SizedBox(height: 8),
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              ElevatedButton(
                onPressed: _handleComputeWeights,
                child: const Text('Compute Weights'),
              ),
              const SizedBox(width: 8),
              ElevatedButton(
                onPressed: () {
                  resetSkeleton();
                  setState(() {
                    _lastBoneIndex = null;
                    _rotationAngle = 0.0;
                  });
                  debugPrint('Skeleton reset');
                },
                child: const Text('Reset Skeleton'),
              ),
              const SizedBox(width: 8),
              ElevatedButton(
                onPressed: () => setState(() => _lastBoneIndex = null),
                child: const Text('Deselect'),
              ),
              const SizedBox(width: 8),
              ElevatedButton(
                onPressed: () => setState(() => _ikMode = !_ikMode),
                child: Text(_ikMode ? 'IK: on' : 'IK: off'),
              ),
              const SizedBox(width: 8),
              ElevatedButton(
                onPressed: () {
                  resetPose();
                  setState(() => _rotationAngle = 0.0);
                },
                child: const Text('Reset Pose'),
              ),
              const SizedBox(width: 16),
              SizedBox(
                width: 160,
                child: Slider(
                  value: _rotationAngle,
                  min: -90,
                  max: 90,
                  label: '${_rotationAngle.toStringAsFixed(0)}°',
                  onChanged: _handleRotationChanged,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _SkeletonOverlayPainter extends CustomPainter {
  _SkeletonOverlayPainter({
    required this.renderSize,
    required this.selectedIndex,
    required Listenable repaint,
  }) : super(repaint: repaint);

  // Index of the currently selected joint, drawn with a ring (or null).
  final int? selectedIndex;

  // Joint positions arrive from Rust in render pixels; the canvas may be
  // displayed at a different size, so everything is scaled by this ratio.
  final double renderSize;

  @override
  void paint(Canvas canvas, Size size) {
    final positions = jointScreenPositions();
    if (positions.isEmpty) return;
    final parents = boneParents();
    final scale = size.width / renderSize;

    final linePaint = Paint()
      ..color = const Color(0xFF3B82F6)
      ..strokeWidth = 2
      ..style = PaintingStyle.stroke;
    final dotFill = Paint()..color = const Color(0xFFF97316);
    final dotOutline = Paint()
      ..color = const Color(0xFF1A1A1A)
      ..strokeWidth = 1.5
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

    final selectedRing = Paint()
      ..color = const Color(0xFFE8E8E8)
      ..strokeWidth = 2.5
      ..style = PaintingStyle.stroke;

    for (var i = 0; i < positions.length; i++) {
      final joint = positions[i];
      if (joint == null) continue;
      final center = toCanvas(joint);
      canvas.drawCircle(center, 6, dotFill);
      canvas.drawCircle(center, 6, dotOutline);
      if (i == selectedIndex) {
        canvas.drawCircle(center, 11, selectedRing);
      }
    }
  }

  @override
  bool shouldRepaint(covariant _SkeletonOverlayPainter oldDelegate) => true;
}
