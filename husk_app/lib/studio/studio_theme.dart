import 'package:flutter/material.dart';

/// Husk's palette, from the branding section of the roadmap.
class HuskColors {
  static const Color canvas = Color(0xFF1A1A1A);
  static const Color app = Color(0xFF1E1E1E);
  static const Color panel = Color(0xFF252526);
  static const Color elevated = Color(0xFF2D2D2D);
  static const Color border = Color(0xFF3C3C3C);

  static const Color textPrimary = Color(0xFFE8E8E8);
  static const Color textSecondary = Color(0xFFA0A0A0);
  static const Color textDisabled = Color(0xFF6B6B6B);

  static const Color blue = Color(0xFF3B82F6);
  static const Color blueHover = Color(0xFF60A5FA);
  static const Color orange = Color(0xFFF97316);
  static const Color orangeHover = Color(0xFFFB923C);
  static const Color success = Color(0xFF22C55E);
  static const Color error = Color(0xFFEF4444);
}

class HuskText {
  static const String ui = 'SpaceGrotesk';
  static const String mono = 'JetBrainsMono';

  /// For numbers and technical readouts: times, counts, values.
  static const TextStyle readout = TextStyle(
    fontFamily: mono,
    fontSize: 12,
    color: HuskColors.textSecondary,
  );
}

ThemeData buildStudioTheme() {
  const scheme = ColorScheme.dark(
    primary: HuskColors.orange,
    onPrimary: HuskColors.canvas,
    secondary: HuskColors.blue,
    onSecondary: Colors.white,
    surface: HuskColors.panel,
    onSurface: HuskColors.textPrimary,
    error: HuskColors.error,
    outline: HuskColors.border,
  );

  final base = ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    colorScheme: scheme,
  );

  return base.copyWith(
    scaffoldBackgroundColor: HuskColors.app,
    dividerColor: HuskColors.border,
    textTheme: base.textTheme.apply(
      fontFamily: HuskText.ui,
      bodyColor: HuskColors.textPrimary,
      displayColor: HuskColors.textPrimary,
    ),
    elevatedButtonTheme: ElevatedButtonThemeData(
      style: ElevatedButton.styleFrom(
        backgroundColor: HuskColors.elevated,
        foregroundColor: HuskColors.textPrimary,
        side: const BorderSide(color: HuskColors.border),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
        textStyle: const TextStyle(fontFamily: HuskText.ui, fontSize: 13),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: HuskColors.orange,
        foregroundColor: HuskColors.canvas,
        disabledBackgroundColor: HuskColors.elevated,
        disabledForegroundColor: HuskColors.textDisabled,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
        textStyle: const TextStyle(
          fontFamily: HuskText.ui,
          fontSize: 13,
          fontWeight: FontWeight.w500,
        ),
      ),
    ),
  );
}