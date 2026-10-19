import 'package:flutter/material.dart';
import 'package:husk_app/src/rust/api/simple.dart'
    show ObjectMaterial, ObjectTransform;
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
            if (studio.activeCameraFov == null)
              CollapsibleSection(
                title: 'Material',
                child: _MaterialSection(studio: studio),
              )
            else
              CollapsibleSection(
                title: 'Camera',
                child: _CameraSection(studio: studio),
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

class _CameraSection extends StatelessWidget {
  const _CameraSection({required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
    final fov = studio.activeCameraFov ?? 35.0;
    return Row(
      children: [
        const SizedBox(
          width: 84,
          child: Text('Field of view', style: HuskText.readout),
        ),
        Expanded(
          child: Slider(
            value: fov.clamp(10.0, 120.0),
            min: 10,
            max: 120,
            onChanged: studio.setActiveCameraFov,
          ),
        ),
        SizedBox(
          width: 40,
          child: Text(
            '${fov.round()}°',
            textAlign: TextAlign.right,
            style: HuskText.readout,
          ),
        ),
      ],
    );
  }
}

/// The colours offered as one-tap swatches: Husk's own, then neutrals and
/// earth tones.
const List<Color> _swatches = [
  Color(0xFFF97316),
  Color(0xFF3B82F6),
  Color(0xFF22C55E),
  Color(0xFFEF4444),
  Color(0xFFE8E8E8),
  Color(0xFF6B6B6B),
  Color(0xFF8B8680),
  Color(0xFF8B5A2B),
  Color(0xFFD9A066),
  Color(0xFFC68642),
  Color(0xFFF1C27D),
  Color(0xFF3B2A20),
];

class _MaterialSection extends StatelessWidget {
  const _MaterialSection({required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
    final material = studio.activeMaterial;
    final (r, g, b) = material.baseColor;

    void change({
      (double, double, double)? color,
      double? roughness,
      double? metalness,
    }) {
      studio.setActiveMaterial(
        ObjectMaterial(
          baseColor: color ?? material.baseColor,
          roughness: roughness ?? material.roughness,
          metalness: metalness ?? material.metalness,
        ),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Text('Colour', style: HuskText.readout),
        const SizedBox(height: 6),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: [
            for (final swatch in _swatches)
              _Swatch(
                color: swatch,
                onTap: () => change(color: (swatch.r, swatch.g, swatch.b)),
              ),
          ],
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            Expanded(
              child: NumberField(
                label: 'R',
                labelColor: HuskColors.error,
                value: (r * 255).roundToDouble(),
                onChanged: (v) => change(color: (v.clamp(0.0, 255.0) / 255, g, b)),
              ),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: NumberField(
                label: 'G',
                labelColor: HuskColors.success,
                value: (g * 255).roundToDouble(),
                onChanged: (v) => change(color: (r, v.clamp(0.0, 255.0) / 255, b)),
              ),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: NumberField(
                label: 'B',
                labelColor: HuskColors.blue,
                value: (b * 255).roundToDouble(),
                onChanged: (v) => change(color: (r, g, v.clamp(0.0, 255.0) / 255)),
              ),
            ),
          ],
        ),
        const SizedBox(height: 10),
        _LabelledSlider(
          label: 'Roughness',
          value: material.roughness,
          onChanged: (v) => change(roughness: v),
        ),
        _LabelledSlider(
          label: 'Metalness',
          value: material.metalness,
          onChanged: (v) => change(metalness: v),
        ),
      ],
    );
  }
}

class _Swatch extends StatelessWidget {
  const _Swatch({required this.color, required this.onTap});

  final Color color;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(4),
      child: Container(
        width: 22,
        height: 22,
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: HuskColors.border),
        ),
      ),
    );
  }
}

class _LabelledSlider extends StatelessWidget {
  const _LabelledSlider({
    required this.label,
    required this.value,
    required this.onChanged,
  });

  final String label;
  final double value;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        SizedBox(width: 74, child: Text(label, style: HuskText.readout)),
        Expanded(
          child: Slider(value: value.clamp(0.0, 1.0), onChanged: onChanged),
        ),
        SizedBox(
          width: 36,
          child: Text(
            value.toStringAsFixed(2),
            textAlign: TextAlign.right,
            style: HuskText.readout,
          ),
        ),
      ],
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
