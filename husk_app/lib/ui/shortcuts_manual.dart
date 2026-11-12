import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// The shortcuts manual: every key and mouse action in Husk in one place,
/// shown by Help > Keyboard Shortcuts (Ctrl+/), one table per group.
///
/// KEEP THIS FILE CURRENT. Whenever a shortcut is added, changed or removed
/// anywhere in the app, change its line here in the same edit.
///
/// Not audited yet: keys inside the timeline, the tool column and the side
/// panels. The tool names below are the ones the code uses; check them
/// against the labels in the tool column.

/// One line of the manual: the keys or mouse action, and what it does.
class ShortcutEntry {
  const ShortcutEntry(this.keys, this.does);

  final String keys;
  final String does;
}

/// A titled group of lines in the manual.
class ShortcutSection {
  const ShortcutSection(this.title, this.entries);

  final String title;
  final List<ShortcutEntry> entries;
}

/// THE MANUAL. Add new shortcuts here.
const List<ShortcutSection> shortcutSections = [
  ShortcutSection('File', [
    ShortcutEntry('Ctrl+S', 'Save the project'),
    ShortcutEntry('Ctrl+O', 'Load a project'),
  ]),
  ShortcutSection('Edit', [
    ShortcutEntry('Ctrl+Z', 'Undo the last bone you placed'),
    ShortcutEntry('Delete', 'Delete the active object'),
  ]),
  ShortcutSection('Move around the view', [
    ShortcutEntry('Middle drag', 'Orbit the view'),
    ShortcutEntry('Shift + middle drag', 'Pan the view'),
    ShortcutEntry('Mouse wheel', 'Zoom in and out'),
    ShortcutEntry('- or Numpad -', 'Zoom out'),
    ShortcutEntry('= or Numpad +', 'Zoom in'),
    ShortcutEntry(
      'Home',
      'Frame All: fit every object in the view (also in the View menu)',
    ),
    ShortcutEntry(
      '. or Numpad .',
      'Frame Selected: fit the active object in the view (also in the View menu)',
    ),
  ]),
  ShortcutSection('Standard views', [
    ShortcutEntry('1', 'Front view'),
    ShortcutEntry('Ctrl+1', 'Back view'),
    ShortcutEntry('3', 'Right view'),
    ShortcutEntry('Ctrl+3', 'Left view'),
    ShortcutEntry('7', 'Top view'),
    ShortcutEntry('Ctrl+7', 'Bottom view'),
    ShortcutEntry('5', 'Switch between perspective and orthographic'),
    ShortcutEntry(
      'Numpad 1, 3, 5, 7',
      'Work like the number keys, with Ctrl for the opposite side',
    ),
  ]),
  ShortcutSection('Window', [
    ShortcutEntry(
      'Ctrl+Space',
      'Maximize the viewport, or bring the panels back',
    ),
    ShortcutEntry('Ctrl+/', 'Open this manual'),
  ]),
  ShortcutSection('Animation', [
    ShortcutEntry('Space', 'Play or pause the animation'),
  ]),
  ShortcutSection('In the viewport', [
    ShortcutEntry('Click a joint dot', 'Select that joint, with any tool'),
    ShortcutEntry(
      'Click an object',
      'Make it the active object (Select, Move object, Rotate object and '
          'Scale object tools)',
    ),
    ShortcutEntry('Click (Place bones tool)', 'Place a bone'),
    ShortcutEntry(
      'Drag a joint (Select tool)',
      'Pose it: swings its parent, or bends the limb above it when IK is '
          'on. Dragging a root joint moves the whole character',
    ),
    ShortcutEntry('Drag (Move tool)', 'Move the whole character'),
    ShortcutEntry(
      'Drag an arrow (Move object tool)',
      'Move the object along that axis',
    ),
    ShortcutEntry(
      'Drag a ring (Rotate object tool)',
      'Turn the object about that axis',
    ),
    ShortcutEntry(
      'Drag a handle (Scale object tool)',
      'Scale along an arrow, or use the centre box to scale all three axes',
    ),
    ShortcutEntry(
      'Drag on empty space',
      'Orbit the view (Select, Place bones and the object tools)',
    ),
    ShortcutEntry(
      'Drop a shape from the tray',
      'Add that shape to the scene where you drop it',
    ),
  ]),
];

/// The manual as a dialog: one table per group, so every group stands apart.
class ShortcutsDialog extends StatelessWidget {
  const ShortcutsDialog({super.key});

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: HuskColors.panel,
      insetPadding: const EdgeInsets.all(24),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 1000, maxHeight: 860),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(28, 18, 12, 12),
              child: Row(
                children: [
                  const Expanded(
                    child: Text(
                      'Keyboard shortcuts',
                      style: TextStyle(
                        fontSize: 22,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                  IconButton(
                    tooltip: 'Close',
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
            ),
            const Divider(height: 1, color: HuskColors.border),
            Flexible(
              child: SingleChildScrollView(
                padding: const EdgeInsets.fromLTRB(28, 4, 28, 28),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    for (final section in shortcutSections)
                      _SectionTable(section),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// One group of the manual: its title, then a bordered table with a header
/// row and a ruled row for every shortcut.
class _SectionTable extends StatelessWidget {
  const _SectionTable(this.section);

  final ShortcutSection section;

  TableRow _headerRow() {
    const style = TextStyle(
      color: HuskColors.textSecondary,
      fontSize: 12,
      fontWeight: FontWeight.w600,
      letterSpacing: 0.8,
    );
    return TableRow(
      decoration: BoxDecoration(color: HuskColors.canvas),
      children: const [
        _Cell(child: Text('SHORTCUT', style: style)),
        _Cell(child: Text('WHAT IT DOES', style: style)),
      ],
    );
  }

  TableRow _entryRow(ShortcutEntry entry) {
    return TableRow(
      children: [
        _Cell(
          child: Align(
            alignment: Alignment.centerLeft,
            child: _KeyChip(entry.keys),
          ),
        ),
        _Cell(
          child: Text(
            entry.does,
            style: const TextStyle(
              color: HuskColors.textSecondary,
              fontSize: 14,
              height: 1.35,
            ),
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 22),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            section.title.toUpperCase(),
            style: const TextStyle(
              color: HuskColors.orange,
              fontSize: 13,
              fontWeight: FontWeight.w700,
              letterSpacing: 1.6,
            ),
          ),
          const SizedBox(height: 10),
          Container(
            clipBehavior: Clip.antiAlias,
            decoration: BoxDecoration(
              border: Border.all(color: HuskColors.border),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Table(
              columnWidths: const {
                0: FixedColumnWidth(300),
                1: FlexColumnWidth(),
              },
              defaultVerticalAlignment: TableCellVerticalAlignment.middle,
              border: const TableBorder(
                horizontalInside: BorderSide(color: HuskColors.border),
                verticalInside: BorderSide(color: HuskColors.border),
              ),
              children: [
                _headerRow(),
                for (final entry in section.entries) _entryRow(entry),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _Cell extends StatelessWidget {
  const _Cell({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      child: child,
    );
  }
}

class _KeyChip extends StatelessWidget {
  const _KeyChip(this.label);

  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
        border: Border.all(color: HuskColors.border),
        borderRadius: BorderRadius.circular(5),
      ),
      child: Text(label, style: HuskText.readout.copyWith(fontSize: 13)),
    );
  }
}