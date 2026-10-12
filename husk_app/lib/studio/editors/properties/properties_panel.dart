import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/widgets/collapsible_section.dart';
import 'package:husk_app/ui/widgets/readout_row.dart';

/// The Properties editor: collapsible sections for whatever is active,
/// each with one clear job. More sections (and tabs, once there are several
/// kinds of thing to inspect) arrive with the sections they belong to.
class PropertiesPanel extends StatelessWidget {
  const PropertiesPanel({super.key, required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    return ListenableBuilder(
      listenable: studio,
      builder: (context, _) {
        return ListView(
          children: [
            CollapsibleSection(
              title: 'Skeleton',
              child: _SkeletonSection(studio: studio),
            ),
            CollapsibleSection(
              title: 'Skin weights',
              child: _WeightsSection(actions: actions),
            ),
            CollapsibleSection(
              title: 'Posing',
              child: _PosingSection(actions: actions),
            ),
            CollapsibleSection(
              title: 'Danger zone',
              initiallyExpanded: false,
              titleColor: HuskColors.error,
              child: _DangerSection(actions: actions),
            ),
          ],
        );
      },
    );
  }
}

class _SkeletonSection extends StatelessWidget {
  const _SkeletonSection({required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
    final selected = studio.selectedBone;
    final parent = selected == null ? null : studio.parentOf(selected);
    final String selectedText = selected == null ? 'none' : 'bone $selected';
    final String parentText = selected == null
        ? 'none'
        : (parent == null ? 'none (root)' : 'bone $parent');
    return Column(
      children: [
        ReadoutRow(label: 'Bones', value: '${studio.boneCount}'),
        ReadoutRow(label: 'Selected', value: selectedText),
        ReadoutRow(label: 'Parent', value: parentText),
      ],
    );
  }
}

class _WeightsSection extends StatelessWidget {
  const _WeightsSection({required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ReadoutRow(
          label: 'Status',
          value: studio.hasWeights ? 'computed' : 'not computed',
        ),
        const SizedBox(height: 8),
        FilledButton(
          onPressed: () => actions.computeWeights(context),
          child: const Text('Compute weights'),
        ),
      ],
    );
  }
}

class _PosingSection extends StatelessWidget {
  const _PosingSection({required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            const Expanded(
              child: Text(
                'IK drag (bend a whole limb)',
                style: TextStyle(fontSize: 13),
              ),
            ),
            Switch(value: studio.ikMode, onChanged: (_) => studio.toggleIk()),
          ],
        ),
        const SizedBox(height: 8),
        ElevatedButton(
          onPressed: actions.resetPose,
          child: const Text('Reset pose'),
        ),
      ],
    );
  }
}

class _DangerSection extends StatelessWidget {
  const _DangerSection({required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Text(
          'Removes every bone, the skin weights and all keys.',
          style: HuskText.readout,
        ),
        const SizedBox(height: 8),
        ElevatedButton(
          style: ElevatedButton.styleFrom(foregroundColor: HuskColors.error),
          onPressed: () => actions.resetSkeleton(context),
          child: const Text('Reset skeleton...'),
        ),
      ],
    );
  }
}
