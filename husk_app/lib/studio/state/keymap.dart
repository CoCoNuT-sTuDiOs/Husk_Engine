import 'package:flutter/services.dart';

/// Every command a key can run. [keyBindings] says which key runs which
/// command, and StudioActions.runCommand says what each command does.
///
/// KEEP lib/ui/shortcuts_manual.dart CURRENT: when a binding changes here,
/// change its line in the manual in the same edit.
enum HuskCommand {
  brushSmaller,
  brushBigger,
  save,
  saveAs,
  load,
  deleteObject,
  duplicate,
  copy,
  paste,
  undo,
  redo,
  zoomIn,
  zoomOut,
  frameAll,
  frameSelected,
  viewFront,
  viewBack,
  viewRight,
  viewLeft,
  viewTop,
  viewBottom,
  toggleOrthographic,
  togglePlay,
  toggleMaximize,
  showShortcuts,
  toolMoveObject,
  toolRotateObject,
  toolScaleObject,
  toggleToolPanel,
  toggleSidePanel,
  toggleMode,
  toggleEditMode,
  extrudeBone,
  cancelGrab,
  renameActive,
}

/// One row of the key table.
class KeyBinding {
  const KeyBinding(
    this.command,
    this.keys, {
    this.ctrl = false,
    this.shift = false,
    this.repeats = false,
    this.whileTyping = false,
  });

  final HuskCommand command;

  /// Any one of these keys runs the command.
  final List<LogicalKeyboardKey> keys;

  /// True when Ctrl must be held, false when it must not be.
  final bool ctrl;

  /// True when Shift must be held. Otherwise Shift is ignored, unless another
  /// row for the same key needs it (Undo is Ctrl+Z, Redo is Ctrl+Shift+Z).
  final bool shift;

  /// True when holding the key down keeps running the command.
  final bool repeats;

  /// True when the command still works while a text box has the keyboard.
  final bool whileTyping;
}

/// THE KEY TABLE. Every keyboard shortcut in Husk is a row here.
const List<KeyBinding> keyBindings = [
  KeyBinding(
    HuskCommand.save,
    [LogicalKeyboardKey.keyS],
    ctrl: true,
    whileTyping: true,
  ),
  KeyBinding(
    HuskCommand.saveAs,
    [LogicalKeyboardKey.keyS],
    ctrl: true,
    shift: true,
    whileTyping: true,
  ),
  KeyBinding(
    HuskCommand.load,
    [LogicalKeyboardKey.keyO],
    ctrl: true,
    whileTyping: true,
  ),
  KeyBinding(
    HuskCommand.undo,
    [LogicalKeyboardKey.keyZ],
    ctrl: true,
    repeats: true,
  ),
  KeyBinding(
    HuskCommand.redo,
    [LogicalKeyboardKey.keyZ],
    ctrl: true,
    shift: true,
    repeats: true,
  ),
KeyBinding(HuskCommand.deleteObject, [LogicalKeyboardKey.delete]),
  KeyBinding(HuskCommand.duplicate, [LogicalKeyboardKey.keyD], shift: true),
  KeyBinding(HuskCommand.copy, [LogicalKeyboardKey.keyC], ctrl: true),
  KeyBinding(HuskCommand.paste, [LogicalKeyboardKey.keyV], ctrl: true),
  KeyBinding(HuskCommand.zoomOut, [
    LogicalKeyboardKey.minus,
    LogicalKeyboardKey.numpadSubtract,
  ], repeats: true),
  KeyBinding(HuskCommand.zoomIn, [
    LogicalKeyboardKey.equal,
    LogicalKeyboardKey.add,
    LogicalKeyboardKey.numpadAdd,
  ], repeats: true),
  KeyBinding(HuskCommand.frameAll, [LogicalKeyboardKey.home]),
  KeyBinding(HuskCommand.frameSelected, [
    LogicalKeyboardKey.numpadDecimal,
    LogicalKeyboardKey.period,
  ]),
  KeyBinding(HuskCommand.viewFront, [
    LogicalKeyboardKey.digit1,
    LogicalKeyboardKey.numpad1,
  ]),
  KeyBinding(HuskCommand.viewBack, [
    LogicalKeyboardKey.digit1,
    LogicalKeyboardKey.numpad1,
  ], ctrl: true),
  KeyBinding(HuskCommand.viewRight, [
    LogicalKeyboardKey.digit3,
    LogicalKeyboardKey.numpad3,
  ]),
  KeyBinding(HuskCommand.viewLeft, [
    LogicalKeyboardKey.digit3,
    LogicalKeyboardKey.numpad3,
  ], ctrl: true),
  KeyBinding(HuskCommand.viewTop, [
    LogicalKeyboardKey.digit7,
    LogicalKeyboardKey.numpad7,
  ]),
  KeyBinding(HuskCommand.viewBottom, [
    LogicalKeyboardKey.digit7,
    LogicalKeyboardKey.numpad7,
  ], ctrl: true),
  KeyBinding(HuskCommand.toggleOrthographic, [
    LogicalKeyboardKey.digit5,
    LogicalKeyboardKey.numpad5,
  ]),
  KeyBinding(HuskCommand.togglePlay, [LogicalKeyboardKey.space]),
  KeyBinding(HuskCommand.toolMoveObject, [LogicalKeyboardKey.keyG]),
  KeyBinding(HuskCommand.toolRotateObject, [LogicalKeyboardKey.keyR]),
  KeyBinding(HuskCommand.toolScaleObject, [LogicalKeyboardKey.keyS]),
  KeyBinding(HuskCommand.toggleToolPanel, [LogicalKeyboardKey.keyT]),
  KeyBinding(HuskCommand.toggleSidePanel, [LogicalKeyboardKey.keyN]),
  KeyBinding(HuskCommand.toggleMode, [LogicalKeyboardKey.tab], ctrl: true),
  KeyBinding(HuskCommand.toggleEditMode, [LogicalKeyboardKey.tab]),
  KeyBinding(HuskCommand.extrudeBone, [LogicalKeyboardKey.keyE]),
  KeyBinding(HuskCommand.cancelGrab, [LogicalKeyboardKey.escape]),
  KeyBinding(HuskCommand.renameActive, [LogicalKeyboardKey.f2]),
  KeyBinding(
    HuskCommand.brushSmaller,
    [LogicalKeyboardKey.bracketLeft],
    repeats: true,
  ),
  KeyBinding(
    HuskCommand.brushBigger,
    [LogicalKeyboardKey.bracketRight],
    repeats: true,
  ),
  KeyBinding(
    HuskCommand.toggleMaximize,
    [LogicalKeyboardKey.space],
    ctrl: true,
    whileTyping: true,
  ),
  KeyBinding(
    HuskCommand.showShortcuts,
    [LogicalKeyboardKey.slash, LogicalKeyboardKey.numpadDivide],
    ctrl: true,
    whileTyping: true,
  ),
];

/// The row a key press runs, or null when the key does nothing.
KeyBinding? bindingForKey(
  LogicalKeyboardKey key, {
  required bool ctrl,
  bool shift = false,
  required bool isRepeat,
}) {
  KeyBinding? ignoringShift;
  for (final binding in keyBindings) {
    if (binding.ctrl != ctrl) continue;
    if (isRepeat && !binding.repeats) continue;
    if (!binding.keys.contains(key)) continue;
    if (binding.shift == shift) return binding;
    if (!binding.shift) ignoringShift ??= binding;
  }
  return ignoringShift;
}
