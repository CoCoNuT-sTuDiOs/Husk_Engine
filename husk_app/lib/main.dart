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
  bool _placeBones = true;
  bool _dragIsRoot = false;
  // Timeline (keyframes). All times are in seconds.
  static const double _timelineLength = 5.0;
  double _time = 0.0;
  bool _playing = false;
  Timer? _playTimer;
  final Stopwatch _playClock = Stopwatch();
  double _playStartTime = 0.0;
  List<double> _keyTimes = [];
  double? _repeatPeriod;
  double _travelSpeed = 0.0;
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
    // A loaded rig means we are posing, not building: start with placing off.
    _placeBones = boneCount() == BigInt.zero;
    _overlayTimer = Timer.periodic(
      const Duration(milliseconds: 33),
      (_) => _overlayTick.value++,
    );
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleKey);
    _playTimer?.cancel();
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

    if (!_placeBones) {
      debugPrint('Bone placement is off; turn on "Place bones" to add joints');
      return;
    }
    // Bones are placed against the rest-pose mesh, so go back to rest first.
    resetPose();

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
    if (success) setState(() => _placeBones = false);
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

  void _refreshKeys() {
    _keyTimes = List<double>.of(keyframeTimes());
    _repeatPeriod = repeatPeriod();
  }

  void _setTime(double time) {
    final clamped = time.clamp(0.0, _timelineLength).toDouble();
    setState(() => _time = clamped);
    seekToTime(time: clamped);
  }

  void _addKey() {
    final time = (_time * 100).round() / 100;
    final count = addKeyframe(time: time);
    setState(_refreshKeys);
    debugPrint('Key set at ${time.toStringAsFixed(2)} s, $count keys in total');
  }

  void _deleteKey() {
    final removed = deleteKeyframeAt(time: _time);
    setState(_refreshKeys);
    debugPrint(removed ? 'Key deleted' : 'No key at this time');
  }

  void _togglePlay() {
    if (_playing) {
      _playTimer?.cancel();
      _playClock.stop();
      setState(() => _playing = false);
      return;
    }
    if (_keyTimes.length < 2) {
      debugPrint('Add at least two keys to play');
      return;
    }
    // With a repeat, play on to the end of the timeline; otherwise stop at the last key.
    final end = _repeatPeriod != null ? _timelineLength : _keyTimes.last;
    // Start over if the playhead is already at the end.
    _playStartTime = _time >= end ? 0.0 : _time;
    _playClock
      ..reset()
      ..start();
    setState(() => _playing = true);
    _playTimer = Timer.periodic(const Duration(milliseconds: 16), (_) {
      final time = _playStartTime + _playClock.elapsedMilliseconds / 1000.0;
      if (time >= end) {
        _playTimer?.cancel();
        _playClock.stop();
        setState(() => _playing = false);
        _setTime(end);
        return;
      }
      _setTime(time);
    });
  }

  void _toggleRepeat() {
    if (_repeatPeriod != null) {
      clearRepeat();
    } else if (!setRepeatEnd(endTime: _time)) {
      debugPrint(
        'Move the playhead past your last key, then press Repeat from here',
      );
      return;
    }
    setState(_refreshKeys);
  }

  Widget _buildTimelineRow() {
    return Wrap(
      spacing: 8,
      runSpacing: 6,
      alignment: WrapAlignment.center,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [        
        ElevatedButton(
          onPressed: _togglePlay,
          child: Text(_playing ? 'Pause' : 'Play'),
        ),
        const SizedBox(width: 8),
        ElevatedButton(onPressed: _addKey, child: const Text('Add Key')),
        const SizedBox(width: 8),
        ElevatedButton(onPressed: _deleteKey, child: const Text('Delete Key')),
        const SizedBox(width: 8),
        ElevatedButton(
          onPressed: _toggleRepeat,
          child: Text(_repeatPeriod == null ? 'Repeat from here' : 'Stop repeat'),
        ),
        const SizedBox(width: 8),
        ElevatedButton(
          onPressed: () => setState(() => _placeBones = !_placeBones),
          child: Text(_placeBones ? 'Place bones: on' : 'Place bones: off'),
        ),
        const SizedBox(width: 12),
        SizedBox(
          width: 320,
          height: 34,
          child: LayoutBuilder(
            builder: (context, constraints) {
              final width = constraints.maxWidth;
              void scrub(Offset position) {
                if (_playing) return;
                _setTime(position.dx / width * _timelineLength);
              }

              return GestureDetector(
                behavior: HitTestBehavior.opaque,
                onTapDown: (details) => scrub(details.localPosition),
                onPanStart: (details) => scrub(details.localPosition),
                onPanUpdate: (details) => scrub(details.localPosition),
                child: CustomPaint(
                  size: Size(width, 34),
                  painter: _TimelinePainter(
                    time: _time,
                    keyTimes: _keyTimes,
                    length: _timelineLength,
                    repeatPeriod: _repeatPeriod,
                  ),
                ),
              );
            },
          ),
        ),
        const SizedBox(width: 8),
        Text('${_time.toStringAsFixed(2)} s'),
        SizedBox(
          width: 260,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Text('Travel'),
              Expanded(
                child: Slider(
                  value: _travelSpeed,
                  min: -2,
                  max: 2,
                  divisions: 40,
                  label: '${_travelSpeed.toStringAsFixed(1)} units/s',
                  onChanged: (value) {
                    setState(() => _travelSpeed = value);
                    setTravelSpeed(speed: value);
                  },
                ),
              ),
              Text('${_travelSpeed.toStringAsFixed(1)}/s'),
            ],
          ),
        ),
        const Text('Green dot = root: drag it to move the character'),
      ],
    );
  }

  bool _handleKey(KeyEvent event) {
    if ((event is KeyDownEvent || event is KeyRepeatEvent) &&
        event.logicalKey == LogicalKeyboardKey.keyZ &&
        HardwareKeyboard.instance.isControlPressed) {
      _handleUndo();
      return true;
    }
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
    final joint = _draggedJoint;
    // A root joint has no parent to swing around: dragging it moves the
    // whole character.
    _dragIsRoot = joint != null && boneParents()[joint.toInt()] == null;
  }

  void _handlePanUpdate(DragUpdateDetails details, double scale) {
    final joint = _draggedJoint;
    if (joint == null) {
      _handleOrbitDrag(details);
      return;
    }
        final x = details.localPosition.dx / scale;
    final y = details.localPosition.dy / scale;
    if (_dragIsRoot) {
      moveCharacter(x: x, y: y);
      return;
    }
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
          _buildTimelineRow(),
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
                  _refreshKeys();
                  _placeBones = true;
                  setState(() {
                    _lastBoneIndex = null;
                    _rotationAngle = 0.0;
                    _travelSpeed = 0.0;
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

class _TimelinePainter extends CustomPainter {
  _TimelinePainter({
    required this.time,
    required this.keyTimes,
    required this.length,
    required this.repeatPeriod,
  });

  final double time;
  final List<double> keyTimes;
  final double length;
  final double? repeatPeriod;
  @override
  void paint(Canvas canvas, Size size) {
    final trackY = size.height / 2;
    final track = Paint()
      ..color = const Color(0xFFB8AEDB)
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round;
    canvas.drawLine(Offset(0, trackY), Offset(size.width, trackY), track);

    // One small tick per second.
    final tick = Paint()
      ..color = const Color(0xFF8E84B8)
      ..strokeWidth = 1;
    for (var second = 0; second <= length.floor(); second++) {
      final x = second / length * size.width;
      canvas.drawLine(Offset(x, trackY + 6), Offset(x, trackY + 12), tick);
    }

    // Keyframes as diamonds.
    final keyFill = Paint()..color = const Color(0xFFF97316);
    final keyOutline = Paint()
      ..color = const Color(0xFF1A1A1A)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    for (final keyTime in keyTimes) {
      final x = keyTime / length * size.width;
      final diamond = Path()
        ..moveTo(x, trackY - 8)
        ..lineTo(x + 7, trackY)
        ..lineTo(x, trackY + 8)
        ..lineTo(x - 7, trackY)
        ..close();
      canvas.drawPath(diamond, keyFill);
      canvas.drawPath(diamond, keyOutline);
    }

    // The repeat: faint copies of the keys, and a line where each repeat starts.
    final period = repeatPeriod;
    if (period != null && keyTimes.isNotEmpty) {
      final ghostFill = Paint()..color = const Color(0x66F97316);
      final boundary = Paint()
        ..color = const Color(0xFF6750A4)
        ..strokeWidth = 1;
      final cycleStart = keyTimes.first;
      for (var repeat = 1; cycleStart + repeat * period <= length; repeat++) {
        final shift = repeat * period;
        final startX = (cycleStart + shift) / length * size.width;
        canvas.drawLine(
          Offset(startX, 4),
          Offset(startX, size.height - 4),
          boundary,
        );
        for (final keyTime in keyTimes) {
          if (keyTime + shift > length) continue;
          final x = (keyTime + shift) / length * size.width;
          final diamond = Path()
            ..moveTo(x, trackY - 8)
            ..lineTo(x + 7, trackY)
            ..lineTo(x, trackY + 8)
            ..lineTo(x - 7, trackY)
            ..close();
          canvas.drawPath(diamond, ghostFill);
        }
      }
    }

    // The playhead.
    final playhead = Paint()
      ..color = const Color(0xFF6750A4)
      ..strokeWidth = 2;
    final headX = (time / length * size.width).clamp(0.0, size.width).toDouble();
    canvas.drawLine(Offset(headX, 2), Offset(headX, size.height - 2), playhead);
  }

  @override
  bool shouldRepaint(covariant _TimelinePainter oldDelegate) =>
      oldDelegate.time != time ||
      oldDelegate.keyTimes != keyTimes ||
      oldDelegate.length != length ||
      oldDelegate.repeatPeriod != repeatPeriod;
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
      final isRoot = i < parents.length && parents[i] == null;
      final radius = isRoot ? 9.0 : 6.0;
      canvas.drawCircle(
        center,
        radius,
        isRoot ? (Paint()..color = const Color(0xFF22C55E)) : dotFill,
      );
      canvas.drawCircle(center, radius, dotOutline);
      if (i == selectedIndex) {
        canvas.drawCircle(center, 11, selectedRing);
      }
    }
  }

  @override
  bool shouldRepaint(covariant _SkeletonOverlayPainter oldDelegate) => true;
}
