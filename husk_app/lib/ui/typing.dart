import 'package:flutter/widgets.dart';

/// True while a text box has the keyboard. Window-wide shortcuts (Delete,
/// the zoom keys, Ctrl+Z) check this and leave the keys to the text box.
bool isTyping() {
  final focus = FocusManager.instance.primaryFocus;
  return focus?.context?.findAncestorWidgetOfExactType<EditableText>() != null;
}