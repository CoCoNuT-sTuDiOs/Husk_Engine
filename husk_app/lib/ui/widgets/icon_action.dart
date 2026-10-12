import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// A small icon-only button with a tooltip: the building block of toolbars
/// and headers. [selected] marks an active toggle (blue); [primary] marks
/// the one main action of a toolbar (orange).
class IconAction extends StatelessWidget {
  const IconAction({
    super.key,
    required this.icon,
    required this.tooltip,
    required this.onPressed,
    this.selected = false,
    this.primary = false,
  });

  final Widget icon;
  final String tooltip;
  final VoidCallback? onPressed;
  final bool selected;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    final Color foreground;
    final Color background;
    final Color outline;
    if (primary) {
      foreground = HuskColors.canvas;
      background = HuskColors.orange;
      outline = Colors.transparent;
    } else if (selected) {
      foreground = HuskColors.blue;
      background = HuskColors.blue.withValues(alpha: 0.2);
      outline = HuskColors.blue;
    } else {
      foreground = onPressed == null
          ? HuskColors.textDisabled
          : HuskColors.textPrimary;
      background = Colors.transparent;
      outline = HuskColors.border;
    }

    return Tooltip(
      message: tooltip,
      child: Material(
        color: background,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(6),
          side: BorderSide(color: outline),
        ),
        child: InkWell(
          borderRadius: BorderRadius.circular(6),
          onTap: onPressed,
          child: SizedBox(
            width: 32,
            height: 32,
            child: IconTheme(
              data: IconThemeData(color: foreground, size: 18),
              child: Center(child: icon),
            ),
          ),
        ),
      ),
    );
  }
}
