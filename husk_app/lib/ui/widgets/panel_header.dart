import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// The title strip at the top of a panel.
class PanelHeader extends StatelessWidget {
  const PanelHeader({super.key, required this.title, this.trailing});

  final String title;
  final Widget? trailing;

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
          Text(
            title,
            style: const TextStyle(
              color: HuskColors.textPrimary,
              fontSize: 13,
              fontWeight: FontWeight.w500,
            ),
          ),
          const Spacer(),
          if (trailing != null) trailing!,
        ],
      ),
    );
  }
}