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
      padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
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

  Path _diamond(double x, double y) {
    return Path()
      ..moveTo(x, y - 8)
      ..lineTo(x + 7, y)
      ..lineTo(x, y + 8)
      ..lineTo(x - 7, y)
      ..close();
  }

  @override
  void paint(Canvas canvas, Size size) {
    final frame = RRect.fromRectAndRadius(
      Offset.zero & size,
      const Radius.circular(6),
    );
    canvas.drawRRect(frame, Paint()..color = HuskColors.canvas);
    canvas.drawRRect(
      frame,
      Paint()
        ..color = HuskColors.border
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1,
    );

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

    final trackY = size.height * 0.62;

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
    canvas.drawPath(
      Path()
        ..moveTo(headX - 6, 0)
        ..lineTo(headX + 6, 0)
        ..lineTo(headX, 9)
        ..close(),
      Paint()..color = HuskColors.blue,
    );
  }

  @override
  bool shouldRepaint(covariant _TimelinePainter oldDelegate) =>
      oldDelegate.time != time ||
      oldDelegate.keyTimes != keyTimes ||
      oldDelegate.length != length ||
      oldDelegate.repeatPeriod != repeatPeriod;
}
