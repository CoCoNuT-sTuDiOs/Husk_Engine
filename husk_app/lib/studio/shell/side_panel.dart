import 'package:flutter/material.dart';
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
    return Container(
      width: 300,
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(left: BorderSide(color: HuskColors.border)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const PanelHeader(title: 'Outliner'),
          const Expanded(
            child: Center(
              child: Text(
                'Objects arrive in Section 5c',
                style: HuskText.readout,
              ),
            ),
          ),
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
