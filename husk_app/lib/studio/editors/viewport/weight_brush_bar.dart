import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// The brush settings bar under the viewport header. It only shows in Weight
/// Paint mode. The starting values are Blender's (weight 1, strength 1,
/// Mix, Accumulate off).
class WeightBrushBar extends StatelessWidget {
  const WeightBrushBar({super.key, required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    return ListenableBuilder(
      listenable: studio,
      builder: (context, _) {
        if (studio.mode != 'weight_paint') return const SizedBox.shrink();
        return Container(
          height: 40,
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: const BoxDecoration(
            color: HuskColors.panel,
            border: Border(bottom: BorderSide(color: HuskColors.border)),
          ),
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                Text(
                  'Bone ${studio.selectedBone ?? BigInt.zero}',
                  style: HuskText.readout,
                ),
                const SizedBox(width: 16),
                _BrushSlider(
                  label: 'Weight',
                  text: studio.brushWeight.toStringAsFixed(2),
                  value: studio.brushWeight,
                  min: 0,
                  max: 1,
                  onChanged: studio.setBrushWeight,
                ),
                const SizedBox(width: 8),
                _BrushSlider(
                  label: 'Strength',
                  text: studio.brushStrength.toStringAsFixed(2),
                  value: studio.brushStrength,
                  min: 0,
                  max: 1,
                  onChanged: studio.setBrushStrength,
                ),
                const SizedBox(width: 8),
                _BrushSlider(
                  label: 'Radius',
                  text: '${studio.brushRadius.round()} px',
                  value: studio.brushRadius,
                  min: 5,
                  max: 200,
                  onChanged: studio.setBrushRadius,
                ),
                const SizedBox(width: 12),
                for (final entry in const [
                  ('mix', 'Mix'),
                  ('add', 'Add'),
                  ('subtract', 'Subtract'),
                  ('smooth', 'Smooth')
                ])
                  Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: ChoiceChip(
                      label: Text(entry.$2),
                      selected: studio.brushBlend == entry.$1,
                      visualDensity: VisualDensity.compact,
                      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      onSelected: (_) => studio.setBrushBlend(entry.$1),
                    ),
                  ),
                const SizedBox(width: 10),
                Tooltip(
                  message:
                      'Off: a stroke never goes past the strength. '
                      'On: staying in one place keeps building up',
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const Text('Accumulate', style: HuskText.readout),
                      const SizedBox(width: 6),
                      Switch(
                        value: studio.brushAccumulate,
                        onChanged: studio.setBrushAccumulate,
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _BrushSlider extends StatelessWidget {
  const _BrushSlider({
    required this.label,
    required this.text,
    required this.value,
    required this.min,
    required this.max,
    required this.onChanged,
  });

  final String label;
  final String text;
  final double value;
  final double min;
  final double max;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(label, style: HuskText.readout),
        SizedBox(
          width: 130,
          child: Slider(
            value: value.clamp(min, max).toDouble(),
            min: min,
            max: max,
            onChanged: onChanged,
          ),
        ),
        SizedBox(width: 46, child: Text(text, style: HuskText.readout)),
      ],
    );
  }
}