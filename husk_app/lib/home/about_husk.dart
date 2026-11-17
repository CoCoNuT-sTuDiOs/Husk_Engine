import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// Shows the About box: the copyright, the licence and the absence of any
/// warranty, as the GNU General Public License asks an interactive program to.
Future<void> showAboutHusk(BuildContext context) {
  return showDialog<void>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      title: const Text('About Husk'),
      content: const SizedBox(
        width: 460,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Husk 0.1', style: HuskText.readout),
            SizedBox(height: 12),
            Text('Copyright (C) 2026 CoCoNuTsTuDiOs'),
            SizedBox(height: 12),
            Text(
              'Husk is free software: you can redistribute it and/or modify it '
              'under the terms of the GNU General Public License as published '
              'by the Free Software Foundation, either version 3 of the '
              'License, or (at your option) any later version.',
            ),
            SizedBox(height: 12),
            Text(
              'Husk comes with ABSOLUTELY NO WARRANTY.',
              style: TextStyle(fontWeight: FontWeight.w600),
            ),
            SizedBox(height: 12),
            Text(
              'Some behaviour is ported from Blender (GPL-2.0-or-later, '
              'Copyright Blender Authors). The files LICENSE and NOTICE in '
              "Husk's folder have the full licence and the list of ported "
              'pieces.',
              style: TextStyle(color: HuskColors.textSecondary, fontSize: 13),
            ),
          ],
        ),
      ),
      actions: [
        FilledButton(
          onPressed: () => Navigator.of(dialogContext).pop(),
          child: const Text('Close'),
        ),
      ],
    ),
  );
}