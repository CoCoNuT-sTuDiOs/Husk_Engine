import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// A small help card that shows the first time something is used, and never
/// again once "Got it" is pressed. Help text goes here, not in the header or
/// other bars, which have no room for sentences.
class FirstTimeHint extends StatelessWidget {
  const FirstTimeHint({
    super.key,
    required this.studio,
    required this.id,
    required this.visibleWhen,
    required this.title,
    required this.lines,
  });

  final StudioController studio;

  /// Names this hint for good. Closing it is remembered under this name.
  final String id;

  /// Whether the thing the hint is about is in use right now.
  final bool Function(StudioController studio) visibleWhen;

  final String title;
  final List<String> lines;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: studio,
      builder: (context, _) {
        if (studio.hintSeen(id) || !visibleWhen(studio)) {
          return const SizedBox.shrink();
        }
        return Container(
          width: 320,
          padding: const EdgeInsets.fromLTRB(16, 14, 16, 12),
          decoration: BoxDecoration(
            color: HuskColors.panel.withValues(alpha: 0.96),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: HuskColors.border),
          ),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                title,
                style: const TextStyle(
                  color: HuskColors.orange,
                  fontSize: 14,
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 8),
              for (final line in lines)
                Padding(
                  padding: const EdgeInsets.only(bottom: 5),
                  child: Text(
                    line,
                    style: const TextStyle(
                      color: HuskColors.textSecondary,
                      fontSize: 13,
                      height: 1.35,
                    ),
                  ),
                ),
              const SizedBox(height: 6),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton(
                  onPressed: () => studio.markHintSeen(id),
                  child: const Text('Got it'),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}