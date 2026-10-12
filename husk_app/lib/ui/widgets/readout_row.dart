import 'package:flutter/material.dart';
import 'package:husk_app/ui/husk_theme.dart';

/// One label and value line, for numbers and technical readouts.
class ReadoutRow extends StatelessWidget {
  const ReadoutRow({super.key, required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        children: [
          SizedBox(
            width: 84,
            child: Text(label, style: HuskText.readout),
          ),
          Expanded(
            child: Text(value, style: const TextStyle(fontSize: 13)),
          ),
        ],
      ),
    );
  }
}
