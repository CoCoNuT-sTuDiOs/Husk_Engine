import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:phosphor_icons/phosphor_icons.dart';

/// A small box with one text field, opened by Edit > Rename active item (F2).
/// It opens ready to type with the current name selected. Enter closes it with
/// the typed text; Esc or a click outside closes it with nothing (null).
class NameDialog extends StatefulWidget {
  const NameDialog({super.key, required this.title, required this.initial});

  /// What is being named, shown above the box ("Bone Name").
  final String title;

  /// The current name, selected so typing replaces it.
  final String initial;

  @override
  State<NameDialog> createState() => _NameDialogState();
}

class _NameDialogState extends State<NameDialog> {
  late final TextEditingController _controller =
      TextEditingController(text: widget.initial)
        ..selection = TextSelection(
          baseOffset: 0,
          extentOffset: widget.initial.length,
        );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: HuskColors.panel,
      insetPadding: const EdgeInsets.all(24),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 300),
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                widget.title,
                style: const TextStyle(
                  color: HuskColors.textSecondary,
                  fontSize: 12,
                ),
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  PhosphorIcon(
                    PhosphorIconsRegular.bone,
                    size: 16,
                    color: HuskColors.textSecondary,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _controller,
                      autofocus: true,
                      style: const TextStyle(fontSize: 13),
                      decoration: const InputDecoration(
                        isDense: true,
                        border: OutlineInputBorder(),
                        contentPadding: EdgeInsets.symmetric(
                          horizontal: 8,
                          vertical: 8,
                        ),
                      ),
                      onSubmitted: (text) => Navigator.of(context).pop(text),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}