import 'package:flutter/material.dart';

/// Shows a short message at the bottom of the window (and in the console).
/// Does nothing when [message] is null.
void showMessage(BuildContext context, String? message) {
  if (message == null) return;
  debugPrint(message);
  final messenger = ScaffoldMessenger.of(context);
  messenger.hideCurrentSnackBar();
  messenger.showSnackBar(
    SnackBar(content: Text(message), duration: const Duration(seconds: 4)),
  );
}
