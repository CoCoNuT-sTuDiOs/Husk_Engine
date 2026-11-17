import 'package:flutter/material.dart';
import 'package:husk_app/studio/editors/outliner/outliner_panel.dart';
import 'package:husk_app/studio/editors/properties/properties_panel.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/widgets/panel_header.dart';

/// The right-hand column: the outliner on top and the properties below.
class SidePanel extends StatelessWidget {
  const SidePanel({super.key, required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
final studio = actions.studio;
    if (studio.sidePanelCollapsed) {
      return Container(
        width: 20,
        decoration: const BoxDecoration(
          color: HuskColors.panel,
          border: Border(left: BorderSide(color: HuskColors.border)),
        ),
        alignment: Alignment.topCenter,
        child: IconButton(
          tooltip: 'Show side panel (N)',
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints.tightFor(width: 20, height: 28),
          icon: const Icon(
            Icons.chevron_left,
            size: 16,
            color: HuskColors.textSecondary,
          ),
          onPressed: studio.toggleSidePanelCollapsed,
        ),
      );
    }
    return Container(
      width: 300,
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(left: BorderSide(color: HuskColors.border)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
        PanelHeader(
            title: 'Outliner',
            trailing: IconButton(
              tooltip: 'Hide side panel (N)',
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints.tightFor(
                width: 24,
                height: 24,
              ),
              icon: const Icon(
                Icons.chevron_right,
                size: 16,
                color: HuskColors.textSecondary,
              ),
              onPressed: studio.toggleSidePanelCollapsed,
            ),
          ),
           Expanded(child: OutlinerPanel(actions: actions)),
          const PanelHeader(title: 'Properties'),
          Expanded(
            flex: 2,
            child: PropertiesPanel(actions: actions),
          ),
        ],
      ),
    );
  }
}
