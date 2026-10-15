import 'dart:async';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/widgets/collapsible_section.dart';

// The engine draws each preview at this many pixels a side; the tray shows
// it smaller, so it stays sharp on high-resolution screens.
const int _previewPixels = 128;
const double _previewSize = 84;

/// The shelf under the viewport: a collapsible row of the built-in shapes,
/// each shown as a render of itself (drawn by the engine, so it matches
/// what lands in the scene). Drag a shape onto the viewport to add it.
class ShapesTray extends StatefulWidget {
  const ShapesTray({super.key, required this.studio});

  final StudioController studio;

  @override
  State<ShapesTray> createState() => _ShapesTrayState();
}

class _ShapesTrayState extends State<ShapesTray> {
  late final List<String> _names = widget.studio.shapeNames;
  final Map<String, ui.Image> _previews = {};

  @override
  void initState() {
    super.initState();
    _loadPreviews();
  }

  @override
  void dispose() {
    for (final image in _previews.values) {
      image.dispose();
    }
    super.dispose();
  }

  /// Asks the engine for each preview in turn, so the tray fills in one
  /// shape at a time and the window never waits.
  Future<void> _loadPreviews() async {
    for (final name in _names) {
      final pixels = await widget.studio.shapePreview(name, _previewPixels);
      if (!mounted) return;
      if (pixels == null) continue;
      final image = await _decode(pixels);
      if (!mounted) {
        image.dispose();
        return;
      }
      setState(() => _previews[name] = image);
    }
  }

  Future<ui.Image> _decode(Uint8List pixels) {
    final done = Completer<ui.Image>();
    ui.decodeImageFromPixels(
      pixels,
      _previewPixels,
      _previewPixels,
      ui.PixelFormat.rgba8888,
      done.complete,
    );
    return done.future;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(top: BorderSide(color: HuskColors.border)),
      ),
      child: CollapsibleSection(
        title: 'Shapes',
        child: SizedBox(
          height: _previewSize + 22,
          child: ListView(
            scrollDirection: Axis.horizontal,
            children: [
              for (final name in _names)
                _ShapeItem(name: name, preview: _previews[name]),
            ],
          ),
        ),
      ),
    );
  }
}

/// One shape in the tray: its render with its name underneath. Until the
/// render arrives it is an empty space of the same size and cannot be
/// dragged.
class _ShapeItem extends StatelessWidget {
  const _ShapeItem({required this.name, required this.preview});

  final String name;
  final ui.Image? preview;

  String get _label => '${name[0].toUpperCase()}${name.substring(1)}';

  Widget _picture() {
    final image = preview;
    return SizedBox(
      width: _previewSize,
      height: _previewSize,
      child: image == null
          ? null
          : RawImage(
              image: image,
              fit: BoxFit.contain,
              filterQuality: FilterQuality.medium,
            ),
    );
  }

  Widget _tile() {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        _picture(),
        const SizedBox(height: 4),
        Text(
          _label,
          style: const TextStyle(
            color: HuskColors.textSecondary,
            fontSize: 11,
          ),
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(right: 20),
      child: preview == null
          ? _tile()
          : MouseRegion(
              cursor: SystemMouseCursors.grab,
              child: Draggable<String>(
                data: name,
                // The preview has a transparent background, so make the
                // whole tile grabbable, not only the painted pixels.
                hitTestBehavior: HitTestBehavior.opaque,
                // The drop position the engine receives is the pointer's own.
                dragAnchorStrategy: pointerDragAnchorStrategy,
                // The shape itself follows the pointer, centred on it, so
                // it shows where the shape will land.
                feedback: Transform.translate(
                  offset: const Offset(-_previewSize / 2, -_previewSize / 2),
                  child: Opacity(opacity: 0.9, child: _picture()),
                ),
                childWhenDragging: Opacity(opacity: 0.35, child: _tile()),
                child: _tile(),
              ),
            ),
    );
  }
}