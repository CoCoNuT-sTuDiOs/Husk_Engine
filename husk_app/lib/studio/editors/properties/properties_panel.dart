import 'package:flutter/material.dart';
import 'package:husk_app/src/rust/api/simple.dart' show ObjectTransform;
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/widgets/collapsible_section.dart';
import 'package:husk_app/ui/widgets/readout_row.dart';
import 'package:husk_app/ui/widgets/number_field.dart';

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
              title: 'Transform',
              child: _TransformSection(studio: studio),
            ),
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

class _TransformSection extends StatelessWidget {
  const _TransformSection({required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
 final t = studio.activeTransform;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _VectorRow(
          label: 'Position',
          value: t.position,
          onChanged: (v) => studio.setActiveTransform(
            ObjectTransform(
              position: v,
              rotationDegrees: t.rotationDegrees,
              scale: t.scale,
            ),
          ),
        ),
        _VectorRow(
          label: 'Rotation (degrees)',
          value: t.rotationDegrees,
          onChanged: (v) => studio.setActiveTransform(
            ObjectTransform(
              position: t.position,
              rotationDegrees: v,
              scale: t.scale,
            ),
          ),
        ),
        _VectorRow(
          label: 'Scale',
          value: t.scale,
          onChanged: (v) => studio.setActiveTransform(
            ObjectTransform(
              position: t.position,
              rotationDegrees: t.rotationDegrees,
              scale: v,
            ),
          ),
        ),
      ],
    );
  }
}

/// A label with three number boxes under it, for X, Y and Z.
class _VectorRow extends StatelessWidget {
  const _VectorRow({
    required this.label,
    required this.value,
    required this.onChanged,
    this.enabled = true,
  });

  final String label;
  final (double, double, double) value;
  final bool enabled;
  final ValueChanged<(double, double, double)> onChanged;

  @override
  Widget build(BuildContext context) {
    final (x, y, z) = value;
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: HuskText.readout),
          const SizedBox(height: 4),
          Row(
            children: [
              Expanded(
                child: NumberField(
                  label: 'X',
                  labelColor: HuskColors.error,
                  value: x,
                  enabled: enabled,
                  onChanged: (v) => onChanged((v, y, z)),
                ),
              ),
              const SizedBox(width: 6),
              Expanded(
                child: NumberField(
                  label: 'Y',
                  labelColor: HuskColors.success,
                  value: y,
                  enabled: enabled,
                  onChanged: (v) => onChanged((x, v, z)),
                ),
              ),
              const SizedBox(width: 6),
              Expanded(
                child: NumberField(
                  label: 'Z',
                  labelColor: HuskColors.blue,
                  value: z,
                  enabled: enabled,
                  onChanged: (v) => onChanged((x, y, v)),
                ),
              ),
            ],
          ),
        ],
      ),
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
