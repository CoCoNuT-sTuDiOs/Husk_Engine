import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/timeline_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// The quick actions floating over the viewport while it fills the window
/// and the panels are hidden: restore the panels, play, set a key, scrub
/// the time, add a camera or light, and delete the active object. Each one
/// runs the same command as its menu entry or panel button.
class MaximizedBar extends StatelessWidget {
  const MaximizedBar({super.key, required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final timeline = actions.timeline;
    return Material(
      color: HuskColors.panel,
      elevation: 4,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
        side: const BorderSide(color: HuskColors.border),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        child: ListenableBuilder(
          listenable: timeline,
          builder: (context, _) => Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextButton(
                onPressed: actions.toggleMaximize,
                child: const Text('Restore panels'),
              ),
              const SizedBox(width: 4),
              TextButton(
                onPressed: () => actions.togglePlay(context),
                child: Text(timeline.playing ? 'Pause' : 'Play'),
              ),
              TextButton(
                onPressed: () => actions.addKey(context),
                child: const Text('Key'),
              ),
              SizedBox(
                width: 160,
                child: Slider(
                  value: timeline.time
                      .clamp(0.0, TimelineController.length)
                      .toDouble(),
                  min: 0,
                  max: TimelineController.length,
                  onChanged: timeline.setTime,
                ),
              ),
              SizedBox(
                width: 52,
                child: Text(
                  '${timeline.time.toStringAsFixed(2)} s',
                  style: HuskText.readout,
                ),
              ),
              const SizedBox(width: 8),
              PopupMenuButton<String>(
                tooltip: 'Add',
                onSelected: (value) {
                  if (value == 'camera') {
                    actions.addCamera(context);
                  } else {
                    actions.addLight(context, value);
                  }
                },
                itemBuilder: (_) => const [
                  PopupMenuItem(value: 'sun', child: Text('Sun')),
                  PopupMenuItem(value: 'point', child: Text('Point light')),
                  PopupMenuItem(value: 'camera', child: Text('Camera')),
                ],
                child: const Padding(
                  padding: EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  child: Text('Add'),
                ),
              ),
              TextButton(
                onPressed: () => actions.deleteObject(context),
                child: const Text('Delete'),
              ),
            ],
          ),
        ),
      ),
    );
  }
}