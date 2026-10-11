import 'package:flutter/material.dart';
import 'package:husk_app/studio/studio_controller.dart';
import 'package:husk_app/studio/studio_theme.dart';
import 'package:husk_app/studio/timeline_controller.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

final ButtonStyle _compactButton = ElevatedButton.styleFrom(
  minimumSize: const Size(0, 32),
  padding: const EdgeInsets.symmetric(horizontal: 12),
);

final ButtonStyle _compactFilledButton = FilledButton.styleFrom(
  minimumSize: const Size(0, 32),
  padding: const EdgeInsets.symmetric(horizontal: 12),
);

/// The timeline panel: transport buttons, key tools, the travel slider,
/// Save and Load, and the scrub track with its keys and playhead.
class TimelinePane extends StatelessWidget {
  const TimelinePane({super.key, required this.timeline, required this.studio});

  final TimelineController timeline;
  final StudioController studio;

  void _show(BuildContext context, String? message) {
    if (message == null) return;
    debugPrint(message);
    final messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    messenger.showSnackBar(
      SnackBar(content: Text(message), duration: const Duration(seconds: 4)),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 210,
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(top: BorderSide(color: HuskColors.border)),
      ),
      child: ListenableBuilder(
        listenable: timeline,
        builder: (context, _) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _TimelineHeader(time: timeline.time),
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 8, 12, 0),
                child: Wrap(
                  spacing: 8,
                  runSpacing: 6,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: [
                    FilledButton.icon(
                      style: _compactFilledButton,
                      onPressed: () => _show(context, timeline.togglePlay()),
                      icon: PhosphorIcon(
                        timeline.playing
                            ? PhosphorIconsRegular.pause
                            : PhosphorIconsRegular.play,
                        size: 16,
                      ),
                      label: Text(timeline.playing ? 'Pause' : 'Play'),
                    ),
                    ElevatedButton.icon(
                      style: _compactButton,
                      onPressed: () => _show(context, timeline.addKey()),
                      icon: PhosphorIcon(PhosphorIconsRegular.diamond, size: 16),
                      label: const Text('Add key'),
                    ),
                    ElevatedButton.icon(
                      style: _compactButton,
                      onPressed: () => _show(context, timeline.deleteKey()),
                      icon: PhosphorIcon(PhosphorIconsRegular.trash, size: 16),
                      label: const Text('Delete key'),
                    ),
                    ElevatedButton.icon(
                      style: _compactButton,
                      onPressed: () => _show(context, timeline.toggleRepeat()),
                      icon: PhosphorIcon(PhosphorIconsRegular.repeat, size: 16),
                      label: Text(
                        timeline.repeatSeconds == null
                            ? 'Repeat from here'
                            : 'Stop repeat',
                      ),
                    ),
                    SizedBox(
                      width: 280,
                      child: Row(
                        children: [
                          const Text('Travel'),
                          Expanded(
                            child: Slider(
                              value: timeline.travel,
                              min: -2,
                              max: 2,
                              divisions: 40,
                              label:
                                  '${timeline.travel.toStringAsFixed(1)} units/s',
                              onChanged: timeline.changeTravel,
                            ),
                          ),
                          Text(
                            '${timeline.travel.toStringAsFixed(1)}/s',
                            style: HuskText.readout,
                          ),
                        ],
                      ),
                    ),
                    ElevatedButton.icon(
                      style: _compactButton,
                      onPressed: () => _show(context, timeline.save()),
                      icon: PhosphorIcon(PhosphorIconsRegular.floppyDisk, size: 16),
                      label: const Text('Save'),
                    ),
                    ElevatedButton.icon(
                      style: _compactButton,
                      onPressed: () {
                        final message = timeline.load();
                        studio.selectBone(null);
                        _show(context, message);
                      },
                      icon: PhosphorIcon(PhosphorIconsRegular.folderOpen, size: 16),
                      label: const Text('Load'),
                    ),
                  ],
                ),
              ),
              Expanded(child: _Track(timeline: timeline)),
            ],
          );
        },
      ),
    );
  }
}

class _TimelineHeader extends StatelessWidget {
  const _TimelineHeader({required this.time});

  final double time;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 32,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: const BoxDecoration(
        color: HuskColors.elevated,
        border: Border(bottom: BorderSide(color: HuskColors.border)),
      ),
      child: Row(
        children: [
          const Text(
            'Timeline',
            style: TextStyle(
              color: HuskColors.textPrimary,
              fontSize: 13,
              fontWeight: FontWeight.w500,
            ),
          ),
          const Spacer(),
          Text('${time.toStringAsFixed(2)} s', style: HuskText.readout),
        ],
      ),
    );
  }
}

class _Track extends StatelessWidget {
  const _Track({required this.timeline});

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

    // The repeat: faint copies of the keys, and a line where each repeat starts.
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
    final headX = (time / length * size.width).clamp(0.0, size.width).toDouble();
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