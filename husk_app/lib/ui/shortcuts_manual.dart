import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// The shortcuts manual: every key and mouse action in Husk in one place,
/// shown by Help > Keyboard Shortcuts (Ctrl+/).
///
/// To my future self =>  KEEP THIS FILE CURRENT. Whenever a shortcut is added, changed or removed
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

/// The manual as a dialog.
class ShortcutsDialog extends StatelessWidget {
  const ShortcutsDialog({super.key});

  List<Widget> _rows(ShortcutSection section) {
    return [
      Padding(
        padding: const EdgeInsets.only(top: 14, bottom: 6),
        child: Text(
          section.title.toUpperCase(),
          style: const TextStyle(
            color: HuskColors.orange,
            fontSize: 12,
            fontWeight: FontWeight.w700,
            letterSpacing: 1.5,
          ),
        ),
      ),
      for (final entry in section.entries)
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 4),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SizedBox(
                width: 210,
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: _KeyChip(entry.keys),
                ),
              ),
              Expanded(
                child: Text(
                  entry.does,
                  style: const TextStyle(
                    color: HuskColors.textSecondary,
                    fontSize: 13,
                  ),
                ),
              ),
            ],
          ),
        ),
    ];
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: HuskColors.panel,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 620, maxHeight: 640),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 12, 8, 8),
              child: Row(
                children: [
                  const Expanded(
                    child: Text(
                      'Keyboard shortcuts',
                      style: TextStyle(
                        fontSize: 18,
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
                padding: const EdgeInsets.fromLTRB(20, 0, 20, 16),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    for (final section in shortcutSections) ..._rows(section),
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

class _KeyChip extends StatelessWidget {
  const _KeyChip(this.label);

  final String label;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        border: Border.all(color: HuskColors.border),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(label, style: HuskText.readout),
    );
  }
}