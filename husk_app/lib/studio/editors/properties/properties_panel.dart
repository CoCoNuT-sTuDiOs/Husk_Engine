import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:husk_app/src/rust/api/simple.dart'
show ObjectMaterial, ObjectTransform, WorldSettings;
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
              if (studio.activeLight != null)
              CollapsibleSection(
                title: 'Light',
                child: _LightSection(studio: studio),
              )
            else if (studio.activeCameraFov == null)
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
              title: 'Output',
            child: _OutputSection(studio: studio),
            ),
            CollapsibleSection(
              title: 'World',
                child: _WorldSection(studio: studio),
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

/// A typed number as a whole number the engine can take (never negative).
int _whole(double value) => value.round().clamp(0, 100000);

/// What the finished video looks like: its shape, size and frame rate. The
/// scene camera frames its shot for this shape.
class _OutputSection extends StatelessWidget {
  const _OutputSection({required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
    final (width, height, fps) = studio.outputSettings;

    Widget preset(String label, int w, int h) => Expanded(
          child: ElevatedButton(
            onPressed: () => studio.setOutputSettings(w, h, fps),
            child: Text(label),
          ),
        );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            preset('16:9', 1920, 1080),
            const SizedBox(width: 6),
            preset('9:16', 1080, 1920),
            const SizedBox(width: 6),
            preset('1:1', 1080, 1080),
          ],
        ),
        const SizedBox(height: 10),
        Row(
          children: [
            Expanded(
              child: NumberField(
                label: 'W',
                value: width.toDouble(),
                onChanged: (v) => studio.setOutputSettings(_whole(v), height, fps),
              ),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: NumberField(
                label: 'H',
                value: height.toDouble(),
                onChanged: (v) => studio.setOutputSettings(width, _whole(v), fps),
              ),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: NumberField(
                label: 'fps',
                value: fps.toDouble(),
                onChanged: (v) => studio.setOutputSettings(width, height, _whole(v)),
              ),
            ),
          ],
        ),
        const SizedBox(height: 6),
        Text(
          'Video: $width x $height at $fps frames per second',
          style: HuskText.readout,
        ),
      ],
    );
  }
}

/// The colours offered for the background: Husk's own dark, blacks and
/// greys, white, a sky blue and a warm sand.
const List<Color> _backgroundSwatches = [
  Color(0xFF0D0D14),
  Color(0xFF000000),
  Color(0xFF2D2D2D),
  Color(0xFF808080),
  Color(0xFFE8E8E8),
  Color(0xFFFFFFFF),
  Color(0xFF87B5E8),
  Color(0xFFF1C27D),
];

/// The space around the objects: the background colour, how strong the
/// soft studio light on surfaces is, and how strong the studio reflections
/// are. It belongs to the whole scene, not to the active object.
class _WorldSection extends StatelessWidget {
  const _WorldSection({required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
    final world = studio.world;
    final (r, g, b) = world.background;

    void change({
      (double, double, double)? background,
      double? ambient,
      double? reflections,
    }) {
      studio.setWorld(
        WorldSettings(
          background: background ?? world.background,
          ambient: ambient ?? world.ambient,
          reflections: reflections ?? world.reflections,
        ),
      );
    }

    Widget strength(String label, double value, ValueChanged<double> onChanged) {
      return Row(
        children: [
          SizedBox(width: 74, child: Text(label, style: HuskText.readout)),
          Expanded(
            child: Slider(
              value: value.clamp(0.0, 2.0),
              min: 0,
              max: 2,
              onChanged: onChanged,
            ),
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        const Text('Background', style: HuskText.readout),
        const SizedBox(height: 6),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: [
            for (final swatch in _backgroundSwatches)
              _Swatch(
                color: swatch,
                onTap: () => change(background: (swatch.r, swatch.g, swatch.b)),
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
                onChanged: (v) =>
                    change(background: (v.clamp(0.0, 255.0) / 255, g, b)),
              ),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: NumberField(
                label: 'G',
                labelColor: HuskColors.success,
                value: (g * 255).roundToDouble(),
                onChanged: (v) =>
                    change(background: (r, v.clamp(0.0, 255.0) / 255, b)),
              ),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: NumberField(
                label: 'B',
                labelColor: HuskColors.blue,
                value: (b * 255).roundToDouble(),
                onChanged: (v) =>
                    change(background: (r, g, v.clamp(0.0, 255.0) / 255)),
              ),
            ),
          ],
        ),
        const SizedBox(height: 10),
        strength('Ambient', world.ambient, (v) => change(ambient: v)),
        strength('Reflections', world.reflections, (v) => change(reflections: v)),
      ],
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

/// A light: its kind, colour and intensity. A sun's intensity is its
/// brightness; a point light's is its brightness one unit away, fading with
/// the square of the distance. The slider runs on a square-root scale, so
/// the small values (where most lights live) are easy to hit.
class _LightSection extends StatelessWidget {
  const _LightSection({required this.studio});

  final StudioController studio;

  @override
  Widget build(BuildContext context) {
    final light = studio.activeLight;
    if (light == null) return const SizedBox.shrink();
    final (r, g, b) = light.color;

    void change({(double, double, double)? color, double? intensity}) {
      studio.setActiveLight(
        color ?? light.color,
        intensity ?? light.intensity,
      );
    }

    final position = light.maxIntensity <= 0
        ? 0.0
        : math
            .sqrt((light.intensity / light.maxIntensity).clamp(0.0, 1.0))
            .toDouble();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ReadoutRow(label: 'Kind', value: light.isSun ? 'Sun' : 'Point light'),
        const SizedBox(height: 8),
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
        Row(
          children: [
            const SizedBox(
              width: 74,
              child: Text('Intensity', style: HuskText.readout),
            ),
            Expanded(
              child: Slider(
                value: position,
                onChanged: (v) => change(intensity: light.maxIntensity * v * v),
              ),
            ),
            SizedBox(
              width: 48,
              child: Text(
                light.intensity.toStringAsFixed(1),
                textAlign: TextAlign.right,
                style: HuskText.readout,
              ),
            ),
          ],
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
