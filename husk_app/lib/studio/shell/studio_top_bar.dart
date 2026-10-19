import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// The top bar: the logo, the File, Edit and View menus, the project name
/// and the Export button.
class StudioTopBar extends StatelessWidget {
  const StudioTopBar({super.key, required this.actions});

  final StudioActions actions;

  Widget _shortcut(String text) => Text(text, style: HuskText.readout);

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 44,
      padding: const EdgeInsets.symmetric(horizontal: 14),
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(bottom: BorderSide(color: HuskColors.border)),
      ),
      child: Row(
        children: [
          Image.asset('assets/images/husk_logo.png', height: 28),
          const SizedBox(width: 10),
          const Text(
            'HUSK',
            style: TextStyle(
              color: HuskColors.orange,
              fontWeight: FontWeight.w700,
              letterSpacing: 2,
              fontSize: 16,
            ),
          ),
          const SizedBox(width: 16),
          Expanded(
            child: ListenableBuilder(
              listenable: actions.studio,
              builder: (context, _) {
                final maximized = actions.studio.viewportMaximized;
                return MenuBar(
                  children: [
                    SubmenuButton(
                      menuChildren: [
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+S'),
                          onPressed: () => actions.save(context),
                          child: const Text('Save project'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+O'),
                          onPressed: () => actions.load(context),
                          child: const Text('Load project'),
                        ),
                      ],
                      child: const Text('File'),
                    ),
                    SubmenuButton(
                      menuChildren: [
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+Z'),
                          onPressed: () => actions.undoBone(context),
                          child: const Text('Undo last bone'),
                        ),
                         MenuItemButton(
                          trailingIcon: _shortcut('Delete'),
                          onPressed: () => actions.deleteObject(context),
                          child: const Text('Delete object'),
                        ),
                        MenuItemButton(
                          onPressed: actions.resetPose,
                          child: const Text('Reset pose'),
                        ),
                        const Divider(height: 8),
                        MenuItemButton(
                          onPressed: () => actions.resetSkeleton(context),
                          child: const Text('Reset skeleton...'),
                        ),
                      ],
                      child: const Text('Edit'),
                    ),
                    SubmenuButton(
                      menuChildren: [
                        MenuItemButton(
                          onPressed: () => actions.addCamera(context),
                          child: const Text('Camera'),
                        ),
                      ],
                      child: const Text('Add'),
                    ),
                    SubmenuButton(
                      menuChildren: [
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+Space'),
                          onPressed: actions.toggleMaximize,
                          child: Text(
                            maximized ? 'Restore panels' : 'Maximize viewport',
                          ),
                        ),
                      ],
                      child: const Text('View'),
                    ),
                  ],
                );
              },
            ),
          ),
          const Text(
            'Untitled project',
            style: TextStyle(color: HuskColors.textSecondary, fontSize: 13),
          ),
          const SizedBox(width: 16),
          // Disabled until Section 7 (export) exists.
          FilledButton.icon(
            onPressed: null,
            icon: PhosphorIcon(PhosphorIconsRegular.export, size: 18),
            label: const Text('Export'),
          ),
        ],
      ),
    );
  }
}
