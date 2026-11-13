import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:husk_app/studio/state/keymap.dart';

void main() {
  test('no key is bound to two commands', () {
    final seen = <String>{};
    for (final binding in keyBindings) {
      for (final key in binding.keys) {
        final id = '${binding.ctrl}-${key.keyId}';
        expect(seen.add(id), isTrue, reason: 'bound twice: $id');
      }
    }
  });

  test('every command has a key', () {
    for (final command in HuskCommand.values) {
      expect(
        keyBindings.any((binding) => binding.command == command),
        isTrue,
        reason: '$command has no key',
      );
    }
  });

  test('Ctrl and key repeat decide which command a key runs', () {
    HuskCommand? commandFor(
      LogicalKeyboardKey key, {
      bool ctrl = false,
      bool isRepeat = false,
    }) => bindingForKey(key, ctrl: ctrl, isRepeat: isRepeat)?.command;

    expect(commandFor(LogicalKeyboardKey.keyS, ctrl: true), HuskCommand.save);
    expect(
      commandFor(LogicalKeyboardKey.keyS),
      HuskCommand.toolScaleObject,
    );
    expect(commandFor(LogicalKeyboardKey.digit1), HuskCommand.viewFront);
    expect(
      commandFor(LogicalKeyboardKey.digit1, ctrl: true),
      HuskCommand.viewBack,
    );
    expect(
      commandFor(LogicalKeyboardKey.minus, isRepeat: true),
      HuskCommand.zoomOut,
    );
    expect(commandFor(LogicalKeyboardKey.delete, isRepeat: true), isNull);
    expect(commandFor(LogicalKeyboardKey.space), HuskCommand.togglePlay);
    expect(
      commandFor(LogicalKeyboardKey.space, ctrl: true),
      HuskCommand.toggleMaximize,
    );
  });
}