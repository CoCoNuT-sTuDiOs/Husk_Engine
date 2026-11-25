import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// The mode menu at the left of the viewport header (Blender's mode selector):
/// the mode the active object is in, and a list of the modes it allows.
class ModeMenu extends StatelessWidget {
  const ModeMenu({super.key, required this.actions});

  final StudioActions actions;

  /// The name shown for a mode.
  static String label(String mode) {
    switch (mode) {
      case 'pose':
      return 'Pose Mode';
      case 'edit':
        return 'Edit Mode';
      case 'weight_paint':
        return 'Weight Paint';
      default:
        return 'Object Mode';
    }
  }

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    final current = studio.mode;
    return PopupMenuButton<String>(
      tooltip: 'Mode (Tab: Object and Edit. Ctrl+Tab: Object and Pose)',
      onSelected: (name) => actions.setMode(context, name),
      itemBuilder: (_) => [
        for (final name in studio.availableModes)
          CheckedPopupMenuItem<String>(
            value: name,
            checked: name == current,
            child: Text(label(name)),
          ),
      ],
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          border: Border.all(color: HuskColors.border),
          borderRadius: BorderRadius.circular(6),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              label(current),
              style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w500),
            ),
            const SizedBox(width: 6),
            const Icon(
              Icons.arrow_drop_down,
              size: 18,
              color: HuskColors.textSecondary,
            ),
          ],
        ),
      ),
    );
  }
}