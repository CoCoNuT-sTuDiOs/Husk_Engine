import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// The vertical strip of tools on the left of the window.
class ToolColumn extends StatelessWidget {
  const ToolColumn({super.key, required this.studio, this.floating = false});

  final StudioController studio;

  /// Whether the column floats over the viewport (while it is maximized)
  /// instead of sitting in the window's left edge.
  final bool floating;

  @override
  Widget build(BuildContext context) {
final collapsed = studio.toolsCollapsed && !floating;
    return Container(
      width: collapsed ? 20 : 52,
      padding: const EdgeInsets.symmetric(vertical: 8),
      decoration: floating
          ? BoxDecoration(
              color: HuskColors.panel.withValues(alpha: 0.92),
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: HuskColors.border),
            )
          : const BoxDecoration(
        color: HuskColors.panel,
        border: Border(right: BorderSide(color: HuskColors.border)),
      ),
      child: ListenableBuilder(
        listenable: studio,
        builder: (context, _) {
          if (studio.toolsCollapsed && !floating) {
            return Align(
              alignment: Alignment.topCenter,
              child: IconButton(
                tooltip: 'Show tool strip (T)',
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 20,
                  height: 28,
                ),
                icon: const Icon(
                  Icons.chevron_right,
                  size: 16,
                  color: HuskColors.textSecondary,
                ),
                onPressed: studio.toggleToolsCollapsed,
              ),
            );
          }
          final allowed = StudioController.toolsInMode(studio.mode);
          return Column(
            // Floating, it is only as tall as its buttons; in the window's
            // left edge it fills the height as before.
            mainAxisSize: floating ? MainAxisSize.min : MainAxisSize.max,
            children: [
              if (!floating)
                IconButton(
                  tooltip: 'Hide tool strip (T)',
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints.tightFor(
                    width: 40,
                    height: 24,
                  ),
                  icon: const Icon(
                    Icons.chevron_left,
                    size: 16,
                    color: HuskColors.textSecondary,
                  ),
                  onPressed: studio.toggleToolsCollapsed,
                ),

              if (allowed.contains(StudioTool.select))
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.cursor),
                tooltip: 'Select',
                selected: studio.tool == StudioTool.select,
                onTap: () => studio.setTool(StudioTool.select),
              ),
              if (allowed.contains(StudioTool.placeBones))
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.bone),
                tooltip: 'Place bones',
                selected: studio.tool == StudioTool.placeBones,
                onTap: () => studio.setTool(StudioTool.placeBones),
              ),
              if (allowed.contains(StudioTool.move))
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.arrowsLeftRight),
                tooltip: 'Move character (animation)',
                selected: studio.tool == StudioTool.move,
                onTap: () => studio.setTool(StudioTool.move),
              ),
              if (allowed.contains(StudioTool.moveObject))
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.arrowsOutCardinal),
                tooltip: 'Move object',
                selected: studio.tool == StudioTool.moveObject,
                onTap: () => studio.setTool(StudioTool.moveObject),
              ),
              if (allowed.contains(StudioTool.rotateObject))
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.arrowsClockwise),
                tooltip: 'Rotate object',
                selected: studio.tool == StudioTool.rotateObject,
                onTap: () => studio.setTool(StudioTool.rotateObject),
              ),
              if (allowed.contains(StudioTool.scaleObject))
              _ToolButton(
                icon: PhosphorIcon(PhosphorIconsRegular.boundingBox),
                tooltip: 'Scale object',
                selected: studio.tool == StudioTool.scaleObject,
                onTap: () => studio.setTool(StudioTool.scaleObject),
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
