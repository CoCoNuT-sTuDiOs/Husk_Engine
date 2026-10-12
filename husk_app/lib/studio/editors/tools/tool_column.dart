import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// The vertical strip of tools on the left of the window.
class ToolColumn extends StatelessWidget {
  const ToolColumn({super.key, required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 52,
      padding: const EdgeInsets.symmetric(vertical: 8),
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(right: BorderSide(color: HuskColors.border)),
      ),
      child: ListenableBuilder(
        listenable: studio,
        builder: (context, _) {
          return Column(
            children: [
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.cursor),
                tooltip: 'Select and pose',
                selected: studio.tool == StudioTool.select,
                onTap: () => studio.setTool(StudioTool.select),
              ),
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.bone),
                tooltip: 'Place bones',
                selected: studio.tool == StudioTool.placeBones,
                onTap: () => studio.setTool(StudioTool.placeBones),
              ),
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.arrowsOutCardinal),
                tooltip: 'Move character',
                selected: studio.tool == StudioTool.move,
                onTap: () => studio.setTool(StudioTool.move),
              ),
            ],
          );
        },
      ),
    );
  }
}

class _ToolButton extends StatelessWidget {
  const _ToolButton({
    required this.icon,
    required this.tooltip,
    required this.selected,
    required this.onTap,
  });

  final Widget icon;
  final String tooltip;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          onTap: onTap,
          child: Container(
            width: 40,
            height: 40,
            margin: const EdgeInsets.symmetric(vertical: 2),
            decoration: BoxDecoration(
              color: selected
                  ? HuskColors.blue.withValues(alpha: 0.2)
                  : Colors.transparent,
              borderRadius: BorderRadius.circular(6),
              border: Border.all(
                color: selected ? HuskColors.blue : Colors.transparent,
              ),
            ),
            child: IconTheme(
              data: IconThemeData(
                color: selected ? HuskColors.blue : HuskColors.textSecondary,
                size: 22,
              ),
              child: Center(child: icon),
            ),
          ),
        ),
      ),
    );
  }
}
