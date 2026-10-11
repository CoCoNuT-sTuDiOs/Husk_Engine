import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:husk_app/src/rust/api/simple.dart' as rust;

/// The timeline's interface state: the playhead, play and pause, and a
/// cached copy of the keys for drawing. The keys themselves live in the
/// Rust core.
class TimelineController extends ChangeNotifier {
  /// TEMPORARY: one fixed project file, like the old canvas. A file picker
  /// arrives with Section 8.
  static const String projectPath =
      r'C:\Users\HomePC\Documents\husk_project.json';

  /// The timeline is a fixed 5 seconds for now (Section 11 makes it
  /// adjustable). All times are in seconds.
  static const double length = 5.0;

  double _time = 0.0;
  bool _playing = false;
  List<double> _keyTimes = const [];
  double? _repeatPeriod;
  double _travelSpeed = 0.0;

  Timer? _playTimer;
  final Stopwatch _playClock = Stopwatch();
  double _playStartTime = 0.0;

  double get time => _time;
  bool get playing => _playing;
  List<double> get keyTimes => _keyTimes;
  double? get repeatSeconds => _repeatPeriod;
  double get travel => _travelSpeed;

  @override
  void dispose() {
    _playTimer?.cancel();
    super.dispose();
  }

  void _refreshKeys() {
    _keyTimes = List<double>.of(rust.keyframeTimes());
    _repeatPeriod = rust.repeatPeriod();
  }

  void setTime(double value) {
    final clamped = value.clamp(0.0, length).toDouble();
    _time = clamped;
    rust.seekToTime(time: clamped);
    notifyListeners();
  }

  String addKey() {
    final at = (_time * 100).round() / 100;
    final count = rust.addKeyframe(time: at);
    _refreshKeys();
    notifyListeners();
    return 'Key set at ${at.toStringAsFixed(2)} s, $count keys in total';
  }

  String deleteKey() {
    final removed = rust.deleteKeyframeAt(time: _time);
    _refreshKeys();
    notifyListeners();
    return removed ? 'Key deleted' : 'No key at this time';
  }

  void _stopPlayback() {
    _playTimer?.cancel();
    _playClock.stop();
    _playing = false;
    notifyListeners();
  }

  /// Starts or stops playback. Returns a message when it can't start.
  String? togglePlay() {
    if (_playing) {
      _stopPlayback();
      return null;
    }
    if (_keyTimes.length < 2) {
      return 'Add at least two keys to play';
    }
    // With a repeat, play on to the end of the timeline; otherwise stop at
    // the last key.
    final end = _repeatPeriod != null ? length : _keyTimes.last;
    // Start over if the playhead is already at the end.
    _playStartTime = _time >= end ? 0.0 : _time;
    _playClock
      ..reset()
      ..start();
    _playing = true;
    notifyListeners();
    _playTimer = Timer.periodic(const Duration(milliseconds: 16), (_) {
      final now = _playStartTime + _playClock.elapsedMilliseconds / 1000.0;
      if (now >= end) {
        _stopPlayback();
        setTime(end);
        return;
      }
      setTime(now);
    });
    return null;
  }

  /// Turns the repeat on at the playhead, or off. Returns a message when
  /// it can't be set.
  String? toggleRepeat() {
    if (_repeatPeriod != null) {
      rust.clearRepeat();
    } else if (!rust.setRepeatEnd(endTime: _time)) {
      return 'Move the playhead past your last key, then press Repeat from here';
    }
    _refreshKeys();
    notifyListeners();
    return null;
  }

  void changeTravel(double value) {
    _travelSpeed = value;
    rust.setTravelSpeed(speed: value);
    notifyListeners();
  }

  String save() => rust.saveProject(path: projectPath).$2;

  String load() {
    if (_playing) _stopPlayback();
    final (ok, message) = rust.loadProject(path: projectPath);
    if (ok) {
      _refreshKeys();
      _time = 0.0;
      _travelSpeed = rust.travelSpeed();
      rust.seekToTime(time: 0.0);
    }
    notifyListeners();
    return message;
  }
}