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
                        MenuItemButton(
                          onPressed: () => actions.importModel(context),
                          child: const Text('Import GLB...'),

                        ),
                      ],
                      child: const Text('File'),
                    ),
                    SubmenuButton(
                      menuChildren: [
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+Z'),
                          onPressed: () => actions.undo(context),
                        child: const Text('Undo'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+Shift+Z'),
                          onPressed: () => actions.redo(context),
                          child: const Text('Redo'),
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
                        MenuItemButton(
                          onPressed: () => actions.addRig(context),
                          child: const Text('Rig'),
                        ),
                        MenuItemButton(
                          onPressed: () => actions.addLight(context, 'sun'),
                          child: const Text('Sun'),
                        ),
                        MenuItemButton(
                          onPressed: () => actions.addLight(context, 'point'),
                          child: const Text('Point light'),
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
                        const Divider(height: 8),
                        MenuItemButton(
                          trailingIcon: _shortcut('Home'),
                          onPressed: () => actions.frameAll(context),
                          child: const Text('Frame All'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('Numpad .'),
                          onPressed: () => actions.frameSelected(context),
                        child: const Text('Frame Selected'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('='),
                          onPressed: actions.zoomIn,
                          child: const Text('Zoom In'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('-'),
                          onPressed: actions.zoomOut,
                          child: const Text('Zoom Out'),
                        ),
                        const Divider(height: 8),
                        MenuItemButton(
                          trailingIcon: _shortcut('1'),
                          onPressed: () => actions.setStandardView('front'),
                          child: const Text('Front'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+1'),
                          onPressed: () => actions.setStandardView('back'),
                          child: const Text('Back'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('3'),
                          onPressed: () => actions.setStandardView('right'),
                          child: const Text('Right'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+3'),
                          onPressed: () => actions.setStandardView('left'),
                          child: const Text('Left'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('7'),
                          onPressed: () => actions.setStandardView('top'),
                          child: const Text('Top'),
                        ),
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+7'),
                          onPressed: () => actions.setStandardView('bottom'),
                          child: const Text('Bottom'),
                        ),
                        const Divider(height: 8),
                        MenuItemButton(
                          trailingIcon: _shortcut('5'),
                          onPressed: actions.toggleOrthographic,
                          child: Text(
                            'Perspective / Orthographic',

                          ),
                        ),
                      ],
                      child: const Text('View'),
                    ),
                    SubmenuButton(
                      menuChildren: [
                        MenuItemButton(
                          trailingIcon: _shortcut('Ctrl+/'),
                          onPressed: () => actions.showShortcuts(context),
                          child: const Text('Keyboard Shortcuts'),
                        ),
                      ],
                      child: const Text('Help'),
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
