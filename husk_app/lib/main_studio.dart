import 'package:flutter/material.dart';
import 'package:husk_app/src/rust/frb_generated.dart';
import 'package:husk_app/studio/shell/studio_shell.dart';
import 'package:husk_app/ui/husk_theme.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const HuskStudioApp());
}

class HuskStudioApp extends StatelessWidget {
  const HuskStudioApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Husk',
      debugShowCheckedModeBanner: false,
      theme: buildHuskTheme(),
      home: const StudioShell(),
    );
  }
}
