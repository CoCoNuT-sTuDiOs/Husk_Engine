import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/widgets/collapsible_section.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// One built-in shape in the tray: the name the engine knows it by (see
/// `by_name` in husk_core/src/geometry/shapes.rs), the label under its
/// icon, and the icon itself.
class _TrayShape {
  const _TrayShape(this.name, this.label, this.icon);

  final String name;
  final String label;
  final IconData icon;
}

const List<_TrayShape> _shapes = [
  _TrayShape('cube', 'Cube', PhosphorIconsRegular.cube),
  _TrayShape('plane', 'Plane', PhosphorIconsRegular.square),
  _TrayShape('capsule', 'Capsule', PhosphorIconsRegular.pill),
  _TrayShape('torus', 'Torus', PhosphorIconsRegular.circleNotch),
];

/// The shelf under the viewport: a collapsible, horizontally scrollable
/// row of shapes. Drag one onto the viewport to add it to the scene.
class ShapesTray extends StatelessWidget {
  const ShapesTray({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(top: BorderSide(color: HuskColors.border)),
      ),
      child: CollapsibleSection(
        title: 'Shapes',
        child: SizedBox(
          height: 72,
          child: ListView(
            scrollDirection: Axis.horizontal,
            children: [
              for (final shape in _shapes) _ShapeTile(shape: shape),
            ],
          ),
        ),
      ),
    );
  }
}

class _ShapeTile extends StatelessWidget {
  const _ShapeTile({required this.shape});

  final _TrayShape shape;

  Widget _face() {
    return Container(
      width: 72,
      height: 72,
      decoration: BoxDecoration(
        color: HuskColors.elevated,
        border: Border.all(color: HuskColors.border),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          PhosphorIcon(
            shape.icon,
            size: 26,
            color: HuskColors.textPrimary,
          ),
          const SizedBox(height: 6),
          Text(
            shape.label,
            style: const TextStyle(
              color: HuskColors.textSecondary,
              fontSize: 11,
            ),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(right: 8),
      child: MouseRegion(
        cursor: SystemMouseCursors.grab,
        child: Draggable<String>(
          data: shape.name,
          // The drop position the engine receives is the pointer's own.
          dragAnchorStrategy: pointerDragAnchorStrategy,
          // Centre the floating tile on the pointer so it shows where the
          // shape will land.
          feedback: Material(
            type: MaterialType.transparency,
            child: Transform.translate(
              offset: const Offset(-36, -36),
              child: Opacity(opacity: 0.85, child: _face()),
            ),
          ),
          childWhenDragging: Opacity(opacity: 0.4, child: _face()),
          child: _face(),
        ),
      ),
    );
  }
}