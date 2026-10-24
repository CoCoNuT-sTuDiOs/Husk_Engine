import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// The outliner: one row per object in the scene. Click a row to make that
/// object the active one; the trash button on a shape's row deletes it.
/// The loaded model (the first row) has no trash button.
class OutlinerPanel extends StatelessWidget {
  const OutlinerPanel({super.key, required this.actions});

  final StudioActions actions;

  @override
  Widget build(BuildContext context) {
    final studio = actions.studio;
    return ListenableBuilder(
      listenable: studio,
      builder: (context, _) {
        final (names, active) = studio.objectList;
        final parents = studio.objectParents;
        final boneParents = studio.objectBoneParents;
        final locks = studio.objectLocks;
        final rows = _treeOrder(parents);
        return ListView.builder(
          itemCount: rows.length,
          itemBuilder: (context, position) {
            final (index, depth) = rows[position];
            final isLocked = index < locks.length && locks[index];
            return _ObjectRow(
                  name: boneParents[index] == null
                  ? names[index]
                  : '${names[index]} (bone ${boneParents[index]})',
              active: index == active,
              depth: depth,
              onTap: () => studio.selectObject(index),
              locked: isLocked,
              onToggleLock: () => actions.setLocked(context, index, !isLocked),
              onDelete: index == 0
                  ? null
                  : () => actions.deleteObjectAt(context, index),
              menuItems: [
                  if (studio.selectedBone != null && index != active)
                  PopupMenuItem<int>(
                    value: -2,
                    child: Text(
                      'Attach to bone ${studio.selectedBone} of ${names[active]}',
                    ),
                  ),
                if (parents[index] != null)
                  const PopupMenuItem<int>(
                    value: -1,
                    child: Text('Free from parent'),
                  ),
                for (var other = 0; other < names.length; other++)
                  if (other != index && !_isBelow(parents, other, index))
                    PopupMenuItem<int>(
                      value: other,
                      child: Text('Parent to ${names[other]}'),
                    ),
              ],
                onMenu: (choice) => choice == -2
                  ? actions.setBoneParent(
                      context,
                      index,
                      active,
                      studio.selectedBone!.toInt(),
                    )
                  : actions.setParent(
                context,
                index,
                choice < 0 ? null : choice,
              ),
            );
          },
        );
      },
    );
  }
}


/// The objects in tree order (each parent followed by its children), each
/// with how deep it sits: (object index, depth).
List<(int, int)> _treeOrder(List<int?> parents) {
  final children = <int, List<int>>{};
  final roots = <int>[];
  for (var i = 0; i < parents.length; i++) {
    final parent = parents[i];
    if (parent == null || parent >= parents.length) {
      roots.add(i);
    } else {
      children.putIfAbsent(parent, () => []).add(i);
    }
  }
  final rows = <(int, int)>[];
  void visit(int index, int depth) {
    rows.add((index, depth));
    for (final child in children[index] ?? const <int>[]) {
      visit(child, depth + 1);
    }
  }

  for (final root in roots) {
    visit(root, 0);
  }
  return rows;
}

/// Whether the object at [object] hangs below the object at [ancestor], at
/// any depth.
bool _isBelow(List<int?> parents, int object, int ancestor) {
  var current = parents[object];
  for (var steps = 0; current != null && steps <= parents.length; steps++) {
    if (current == ancestor) return true;
    current = parents[current];
  }
  return false;
}

class _ObjectRow extends StatelessWidget {
  const _ObjectRow({
    required this.name,
    required this.active,
    required this.onTap,
    required this.onDelete,
    required this.depth,
    required this.menuItems,
    required this.locked,
    required this.onToggleLock,
    required this.onMenu,
  });

  final String name;
  final bool active;
  final VoidCallback onTap;
  final VoidCallback? onDelete;

  /// Whether the object is locked (cannot be clicked in the viewport,
  /// moved or deleted).
  final bool locked;
  final VoidCallback onToggleLock;

  /// How many parents this object hangs below; the row is indented by it.
  final int depth;

  /// The choices in the row's menu: -1 frees the object, any other number
  /// is the index of the object to parent it to.
  final List<PopupMenuEntry<int>> menuItems;
  final ValueChanged<int> onMenu;
  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Container(
        height: 30,
        padding: EdgeInsets.only(left: 12.0 + depth * 14.0, right: 4),
        color: active ? HuskColors.elevated : Colors.transparent,
        child: Row(
          children: [
            PhosphorIcon(
              PhosphorIconsRegular.cube,
              size: 14,
              color: active ? HuskColors.orange : HuskColors.textSecondary,
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                name,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: active
                      ? HuskColors.textPrimary
                      : HuskColors.textSecondary,
                  fontSize: 12,
                ),
              ),
            ),
            IconButton(
              tooltip: locked ? 'Unlock' : 'Lock',
              padding: EdgeInsets.zero,
              constraints: const BoxConstraints.tightFor(width: 26, height: 26),
              icon: Icon(
                locked ? Icons.lock_outline : Icons.lock_open_outlined,
                size: 14,
                color: locked ? HuskColors.orange : HuskColors.textSecondary,
              ),
              onPressed: onToggleLock,
            ),
            PopupMenuButton<int>(
              tooltip: 'Parent',
              padding: EdgeInsets.zero,
              iconSize: 14,
              enabled: menuItems.isNotEmpty,
              icon: Icon(
                Icons.account_tree_outlined,
                size: 14,
                color: HuskColors.textSecondary,
              ),
              itemBuilder: (_) => menuItems,
              onSelected: onMenu,
            ),
            if (onDelete != null)
              IconButton(
                tooltip: 'Delete',
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints.tightFor(
                  width: 26,
                  height: 26,
                ),
                icon: PhosphorIcon(
                  PhosphorIconsRegular.trash,
                  size: 14,
                  color: HuskColors.textSecondary,
                ),
                onPressed: onDelete,
              ),
          ],
        ),
      ),
    );
  }
}