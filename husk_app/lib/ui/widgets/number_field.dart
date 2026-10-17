import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// A small numeric entry box: type a number, then press Enter, press Tab
/// or click away to apply it. Anything that is not a finite number snaps
/// back to the current value. [label] is a short letter shown inside the
/// box, such as an axis.
class NumberField extends StatefulWidget {
  const NumberField({
    super.key,
    required this.value,
    required this.onChanged,
    this.label,
    this.labelColor = HuskColors.textSecondary,
    this.enabled = true,
  });

  final double value;
  final ValueChanged<double> onChanged;
  final String? label;
  final Color labelColor;
  final bool enabled;

  @override
  State<NumberField> createState() => _NumberFieldState();
}

class _NumberFieldState extends State<NumberField> {
  late final TextEditingController _controller =
      TextEditingController(text: _format(widget.value));
  final FocusNode _focus = FocusNode();

  /// Up to three decimals without trailing zeros: 1.5, 0.4, 90.
  static String _format(double value) {
    var text = value.toStringAsFixed(3);
    if (text.contains('.')) {
      text = text.replaceFirst(RegExp(r'0+$'), '');
      text = text.replaceFirst(RegExp(r'\.$'), '');
    }
    return text == '-0' ? '0' : text;
  }

  @override
  void initState() {
    super.initState();
    _focus.addListener(_handleFocus);
  }

  @override
  void didUpdateWidget(NumberField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (!_focus.hasFocus && oldWidget.value != widget.value) {
      _controller.text = _format(widget.value);
    }
  }

  @override
  void dispose() {
    _focus.removeListener(_handleFocus);
    _focus.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _handleFocus() {
    if (_focus.hasFocus) {
      _controller.selection = TextSelection(
        baseOffset: 0,
        extentOffset: _controller.text.length,
      );
    } else {
      _commit();
    }
  }

  void _commit() {
    final parsed = double.tryParse(
      _controller.text.trim().replaceAll(',', '.'),
    );
    if (parsed != null && parsed.isFinite && parsed != widget.value) {
      widget.onChanged(parsed);
      // Once the new value has come back down, show it as the engine holds
      // it (an entry the engine adjusted shows the adjusted number).
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && !_focus.hasFocus) {
          _controller.text = _format(widget.value);
        }
      });
    } else {
      // Not a number, or unchanged: show the current value again.
      _controller.text = _format(widget.value);
    }
  }

  @override
  Widget build(BuildContext context) {
    final border = OutlineInputBorder(
      borderRadius: BorderRadius.circular(4),
      borderSide: const BorderSide(color: HuskColors.border),
    );
    final label = widget.label;
    return SizedBox(
      height: 30,
      child: TextField(
        controller: _controller,
        focusNode: _focus,
        enabled: widget.enabled,
        keyboardType: const TextInputType.numberWithOptions(
          decimal: true,
          signed: true,
        ),
        style: TextStyle(
          fontFamily: HuskText.mono,
          fontSize: 12,
          color: widget.enabled
              ? HuskColors.textPrimary
              : HuskColors.textDisabled,
        ),
        onSubmitted: (_) => _focus.unfocus(),
        decoration: InputDecoration(
          isDense: true,
          filled: true,
          fillColor: HuskColors.elevated,
          contentPadding: const EdgeInsets.symmetric(
            horizontal: 8,
            vertical: 8,
          ),
          prefix: label == null
              ? null
              : Padding(
                  padding: const EdgeInsets.only(right: 6),
                  child: Text(
                    label,
                    style: TextStyle(
                      color: widget.labelColor,
                      fontSize: 11,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
          border: border,
          enabledBorder: border,
          disabledBorder: border,
          focusedBorder: border.copyWith(
            borderSide: const BorderSide(color: HuskColors.orange),
          ),
        ),
      ),
    );
  }
}