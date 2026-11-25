import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/timeline_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// The scrub track: seconds, keyframe diamonds, the repeat and the playhead.
/// Tapping or dragging on it moves the playhead.
class TimelineTrack extends StatelessWidget {
  const TimelineTrack({super.key, required this.timeline});

  final TimelineController timeline;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: EdgeInsets.zero,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final width = constraints.maxWidth;

          void scrub(Offset position) {
            if (timeline.playing) return;
            timeline.setTime(position.dx / width * TimelineController.length);
          }

          return GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTapDown: (details) => scrub(details.localPosition),
            onPanStart: (details) => scrub(details.localPosition),
            onPanUpdate: (details) => scrub(details.localPosition),
            child: CustomPaint(
              size: Size(width, constraints.maxHeight),
              painter: _TimelinePainter(
                time: timeline.time,
                keyTimes: timeline.keyTimes,
                length: TimelineController.length,
                repeatPeriod: timeline.repeatSeconds,
              ),
            ),
          );
        },
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

static const double _rulerHeight = 20;

  Path _diamond(double x, double y) {
    return Path()
      ..moveTo(x, y - 5)
      ..lineTo(x + 5, y)
      ..lineTo(x, y + 5)
      ..lineTo(x - 5, y)
      ..close();
  }

  @override
  void paint(Canvas canvas, Size size) {
    final frame = RRect.fromRectAndRadius(
      Offset.zero & size,
      Radius.zero,
    );
    canvas.drawRRect(frame, Paint()..color = HuskColors.canvas);
    canvas.drawRRect(
      frame,
      Paint()
        ..color = HuskColors.border
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1,
    );

// The ruler along the top, as in Blender's timeline: the seconds as
    // numbers and a small tick every quarter second.
    canvas.drawRect(
      Rect.fromLTWH(0, 0, size.width, _rulerHeight),
      Paint()..color = HuskColors.elevated,
    );
    final tick = Paint()
      ..color = HuskColors.textDisabled
      ..strokeWidth = 1;
    for (var quarter = 0; quarter <= length * 4; quarter++) {
      if (quarter % 4 == 0) continue;
      final x = quarter / 4 / length * size.width;
      canvas.drawLine(
        Offset(x, _rulerHeight - 5),
        Offset(x, _rulerHeight),
        tick,
      );
    }

    // One gridline and label per second.
    final gridline = Paint()
      ..color = HuskColors.border
      ..strokeWidth = 1;
    for (var second = 0; second <= length.floor(); second++) {
      final x = second / length * size.width;
      canvas.drawLine(Offset(x, 0), Offset(x, size.height), gridline);
      final label = TextPainter(
        text: TextSpan(text: '${second}s', style: HuskText.readout),
        textDirection: TextDirection.ltr,
      )..layout();
      label.paint(canvas, Offset(x + 4, 4));
    }


  final trackY = _rulerHeight + (size.height - _rulerHeight) / 2;

    // Keyframes as diamonds.
    final keyFill = Paint()..color = HuskColors.orange;
    final keyOutline = Paint()
      ..color = HuskColors.canvas
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.5;
    for (final keyTime in keyTimes) {
      final x = keyTime / length * size.width;
      final diamond = _diamond(x, trackY);
      canvas.drawPath(diamond, keyFill);
      canvas.drawPath(diamond, keyOutline);
    }

    // The repeat: faint copies of the keys, and a line where each repeat
    // starts.
    final period = repeatPeriod;
    if (period != null && keyTimes.isNotEmpty) {
      final ghostFill = Paint()
        ..color = HuskColors.orange.withValues(alpha: 0.4);
      final boundary = Paint()
        ..color = HuskColors.blue.withValues(alpha: 0.6)
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
          canvas.drawPath(_diamond(x, trackY), ghostFill);
        }
      }
    }

    // The playhead.
    final headX =
        (time / length * size.width).clamp(0.0, size.width).toDouble();
    canvas.drawLine(
      Offset(headX, 0),
      Offset(headX, size.height),
      Paint()
        ..color = HuskColors.blue
        ..strokeWidth = 2,
    );
    final stamp = TextPainter(
      text: TextSpan(
        text: time.toStringAsFixed(2),
        style: HuskText.readout.copyWith(color: Colors.white, fontSize: 11),
      ),
      textDirection: TextDirection.ltr,
    )..layout();
    final boxCentre = Offset(
      headX
          .clamp(stamp.width / 2 + 5, size.width - stamp.width / 2 - 5)
          .toDouble(),
      _rulerHeight / 2,
    );
    final box = RRect.fromRectAndRadius(
      Rect.fromCenter(
        center: boxCentre,
        width: stamp.width + 10,
        height: _rulerHeight - 4,
      ),
      const Radius.circular(3),
    );
    canvas.drawRRect(box, Paint()..color = HuskColors.blue);
    stamp.paint(
      canvas,
      Offset(boxCentre.dx - stamp.width / 2, boxCentre.dy - stamp.height / 2),
    );

  }

  @override
  bool shouldRepaint(covariant _TimelinePainter oldDelegate) =>
      oldDelegate.time != time ||
      oldDelegate.keyTimes != keyTimes ||
      oldDelegate.length != length ||
      oldDelegate.repeatPeriod != repeatPeriod;
}
