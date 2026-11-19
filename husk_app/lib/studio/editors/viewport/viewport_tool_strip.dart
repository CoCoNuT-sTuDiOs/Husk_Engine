import 'package:flutter/material.dart';
import 'package:husk_app/studio/editors/viewport/mode_menu.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/widgets/icon_action.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// The thin bar above the viewport (Blender calls it the header): the
/// active tool's name and options, and the maximize button.
class ViewportToolStrip extends StatelessWidget {
  const ViewportToolStrip({super.key, required this.actions});

  final StudioActions actions;

  String _toolName(StudioTool tool) {
    switch (tool) {
      case StudioTool.select:
        return 'Select';
      case StudioTool.placeBones:
        return 'Place bones';
      case StudioTool.move:
        return 'Move character';
      case StudioTool.moveObject:
        return 'Move object';
      case StudioTool.rotateObject:
        return 'Rotate object';
      case StudioTool.scaleObject:
        return 'Scale object';
    }
  }

  List<Widget> _options(BuildContext context, StudioController studio) {
        if (studio.mode == 'weight_paint') {
      return [
        const Text(
          'Weight Paint: the brush arrives in the next step',
          style: HuskText.readout,
        ),
      ];
    }
    if (studio.tool == StudioTool.select && studio.mode != 'pose') {
      return [
        const Text(
          'Click an object to select it. Ctrl+Tab for Pose Mode',
          style: HuskText.readout,
        ),
      ];
    }
    switch (studio.tool) {
      case StudioTool.select:
        return [
          const Text('IK drag', style: HuskText.readout),
          const SizedBox(width: 6),
          Switch(value: studio.ikMode, onChanged: (_) => studio.toggleIk()),
        ];
      case StudioTool.placeBones:
        final parent = studio.selectedBone;
        return [
          Text(
            parent == null ? 'Parent: none (new root)' : 'Parent: bone $parent',
            style: HuskText.readout,
          ),
          const SizedBox(width: 10),
          IconAction(
            icon: PhosphorIcon(PhosphorIconsRegular.arrowCounterClockwise),
            tooltip: 'Undo (Ctrl+Z)',
          onPressed: () => actions.undo(context),
          ),
        ];
      case StudioTool.move:
        return [
          const Text(
            'Drag anywhere to move the whole character',
            style: HuskText.readout,
          ),
        ];
      case StudioTool.moveObject:
        return [
          const Text(
            'Drag an arrow to move the active object along that axis',
            style: HuskText.readout,
          ),
        ];
      case StudioTool.rotateObject:
        return [
          const Text(
            'Drag a ring to turn the active object (orbit if a ring is edge-on)',
            style: HuskText.readout,
          ),
        ];
      case StudioTool.scaleObject:
        return [
          const Text(
            'Drag a handle to scale along that axis, or the centre box to scale all three',
            style: HuskText.readout,
          ),
        ];
    }

  }

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    return Container(
      height: 40,
      padding: const EdgeInsets.symmetric(horizontal: 10),
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(bottom: BorderSide(color: HuskColors.border)),
      ),
      child: ListenableBuilder(
        listenable: studio,
        builder: (context, _) {
          final maximized = studio.viewportMaximized;
          return Row(
            children: [
               ModeMenu(actions: actions),
              const SizedBox(width: 14),
              Text(
                _toolName(studio.tool),
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
              ),
              const SizedBox(width: 18),
              ..._options(context, studio),
            const Spacer(),
              Tooltip(
                message:
                    'Snap to surface: shapes you drop, and the active object '
                    'you drag with Move object, stick to the surface under '
                    'the cursor',
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const Text('Snap', style: HuskText.readout),
                    const SizedBox(width: 6),
                    Switch(
                      value: studio.snapToSurface,
                      onChanged: (_) => studio.toggleSnapToSurface(),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 12),
              IconAction(
                selected: studio.cameraView,
                icon: PhosphorIcon(PhosphorIconsRegular.videoCamera),
                tooltip: studio.cameraView
                    ? 'Leave camera view'
                    : 'Look through the scene camera',
                onPressed: () => actions.toggleCameraView(context),
              ),
              const SizedBox(width: 6),
              IconAction(
                icon: PhosphorIcon(
                  maximized
                      ? PhosphorIconsRegular.arrowsIn
                      : PhosphorIconsRegular.arrowsOut,
                ),
                tooltip: maximized
                    ? 'Restore panels (Ctrl+Space)'
                    : 'Maximize viewport (Ctrl+Space)',
                onPressed: actions.toggleMaximize,
              ),
            ],
          );
        },
      ),
    );
  }
}
