import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:husk_app/ui/shortcuts_manual.dart';

void main() {
  test('every shortcut has keys and a description', () {
    for (final section in shortcutSections) {
      expect(section.title.trim(), isNotEmpty);
      expect(section.entries, isNotEmpty, reason: section.title);
      for (final entry in section.entries) {
        expect(entry.keys.trim(), isNotEmpty, reason: section.title);
        expect(entry.does.trim(), isNotEmpty, reason: entry.keys);
      }
    }
  });

  test('no key is listed twice, which would mean two commands share it', () {
    final seen = <String>{};
    for (final section in shortcutSections) {
      for (final entry in section.entries) {
        expect(
          seen.add(entry.keys),
          isTrue,
          reason: '${entry.keys} is listed twice',
        );
      }
    }
  });

  testWidgets('the manual shows its title and every section', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(home: Scaffold(body: ShortcutsDialog())),
    );
    expect(find.text('Keyboard shortcuts'), findsOneWidget);
    for (final section in shortcutSections) {
      expect(find.text(section.title.toUpperCase()), findsOneWidget);
    }
  });
}