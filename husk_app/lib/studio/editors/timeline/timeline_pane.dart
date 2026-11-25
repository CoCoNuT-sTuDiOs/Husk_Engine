import 'package:flutter/material.dart';
import 'package:husk_app/studio/editors/timeline/timeline_track.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/widgets/icon_action.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// The timeline editor: a header with the transport and key buttons, the
/// travel setting and the time, and the scrub track underneath.
class TimelinePane extends StatelessWidget {
  const TimelinePane({super.key, required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final timeline = actions.timeline;
    return Container(
    // The height comes from the content: the header, and the track unless folded.
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(top: BorderSide(color: HuskColors.border)),
      ),
      child: ListenableBuilder(
        listenable: timeline,
        builder: (context, _) {
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _TimelineHeader(actions: actions),
              if (!timeline.collapsed)
              SizedBox(height: 40, child: TimelineTrack(timeline: timeline)),
            ],
          );
        },
      ),
    );
  }
}

class _TimelineHeader extends StatelessWidget {
  const _TimelineHeader({required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final timeline = actions.timeline;
    final repeating = timeline.repeatSeconds != null;
    return Container(
      height: 30,
      padding: const EdgeInsets.symmetric(horizontal: 10),
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
          const SizedBox(width: 16),
          IconAction(
            primary: true,
            icon: PhosphorIcon(
              timeline.playing
                  ? PhosphorIconsRegular.pause
                  : PhosphorIconsRegular.play,
            ),
            tooltip: timeline.playing ? 'Pause' : 'Play',
            onPressed: () => actions.togglePlay(context),
          ),
          const SizedBox(width: 6),
          IconAction(
            icon: PhosphorIcon(PhosphorIconsRegular.diamond),
            tooltip: 'Add a key at the playhead',
            onPressed: () => actions.addKey(context),
          ),
          const SizedBox(width: 6),
          IconAction(
            icon: PhosphorIcon(PhosphorIconsRegular.trash),
            tooltip: 'Delete the key at the playhead',
            onPressed: () => actions.deleteKey(context),
          ),
          const SizedBox(width: 6),
          IconAction(
            selected: repeating,
            icon: PhosphorIcon(PhosphorIconsRegular.repeat),
            tooltip: repeating ? 'Stop repeat' : 'Repeat from the playhead',
          onPressed: () => actions.toggleRepeat(context),
          ),
          const SizedBox(width: 6),
          IconAction(
            icon: Icon(
              timeline.collapsed ? Icons.expand_less : Icons.expand_more,
            ),
            tooltip: timeline.collapsed ? 'Show the track' : 'Fold the track',
            onPressed: timeline.toggleCollapsed,
          ),
          const Spacer(),
          const Text('Travel', style: HuskText.readout),
          SizedBox(
            width: 160,
            child: SliderTheme(
              data: SliderTheme.of(context).copyWith(
                trackHeight: 3,
                thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
                overlayShape: const RoundSliderOverlayShape(overlayRadius: 12),
              ),
              child: Slider(
              value: timeline.travel,
              min: -2,
              max: 2,
              divisions: 40,
              label: '${timeline.travel.toStringAsFixed(1)} units/s',
              onChanged: timeline.changeTravel,
              ),
            ),
          ),
          SizedBox(
            width: 52,
            child: Text(
              '${timeline.travel.toStringAsFixed(1)}/s',
              textAlign: TextAlign.right,
              style: HuskText.readout,
            ),
          ),
          const SizedBox(width: 16),
          Text(
            '${timeline.time.toStringAsFixed(2)} s',
            style: HuskText.readout,
          ),
        ],
      ),
    );
  }
}
