import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// The outliner: one row per object in the scene. Click a row to make that
/// object the active one; the trash button on a shape's row deletes it.
/// The loaded model (the first row) has no trash button.
class OutlinerPanel extends StatelessWidget {
  const OutlinerPanel({super.key, required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    return ListenableBuilder(
      listenable: studio,
      builder: (context, _) {
        final (names, active) = studio.objectList;
        return ListView.builder(
          itemCount: names.length,
          itemBuilder: (context, index) => _ObjectRow(
            name: names[index],
            active: index == active,
            onTap: () => studio.selectObject(index),
            onDelete: index == 0
                ? null
                : () => actions.deleteObjectAt(context, index),
          ),
        );
      },
    );
  }
}

class _ObjectRow extends StatelessWidget {
  const _ObjectRow({
    required this.name,
    required this.active,
    required this.onTap,
    required this.onDelete,
  });

  final String name;
  final bool active;
  final VoidCallback onTap;
  final VoidCallback? onDelete;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        height: 30,
        padding: const EdgeInsets.only(left: 12, right: 4),
        color: active ? HuskColors.elevated : Colors.transparent,
        child: Row(
          children: [
            PhosphorIcon(
              PhosphorIconsRegular.cube,
              size: 14,
              color: active ? HuskColors.orange : HuskColors.textSecondary,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                name,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: active
                      ? HuskColors.textPrimary
                      : HuskColors.textSecondary,
                  fontSize: 12,
                ),
              ),
            ),
            if (onDelete != null)
              IconButton(
                tooltip: 'Delete',
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 26,
                  height: 26,
                ),
                icon: PhosphorIcon(
                  PhosphorIconsRegular.trash,
                  size: 14,
                  color: HuskColors.textSecondary,
                ),
                onPressed: onDelete,
              ),
          ],
        ),
      ),
    );
  }
}