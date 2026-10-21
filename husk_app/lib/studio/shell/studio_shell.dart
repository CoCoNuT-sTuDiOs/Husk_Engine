import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/studio/editors/shapes/shapes_tray.dart';
import 'package:husk_app/studio/editors/timeline/timeline_pane.dart';
import 'package:husk_app/studio/editors/tools/tool_column.dart';
import 'package:husk_app/studio/editors/viewport/viewport_pane.dart';
import 'package:husk_app/studio/shell/side_panel.dart';
import 'package:husk_app/studio/shell/studio_top_bar.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/timeline_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/typing.dart';

class StudioShell extends StatefulWidget {
  const StudioShell({super.key});

  @override
  State<StudioShell> createState() => _StudioShellState();
}

class _StudioShellState extends State<StudioShell> {
  final StudioController _studio = StudioController();
  final TimelineController _timeline = TimelineController();
  final ViewportController _viewport = ViewportController();
  late final StudioActions _actions = StudioActions(
    studio: _studio,
    timeline: _timeline,
    viewport: _viewport,
  );

  // Keeps the viewport's state (texture, size) when it moves between the
  // normal layout and the maximized one.
  final GlobalKey _viewportKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    HardwareKeyboard.instance.addHandler(_handleKey);
    _lastActiveObject = _studio.objectList.$2;
    _studio.addListener(_onStudioChanged);
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleKey);
    _studio.removeListener(_onStudioChanged);
    _studio.dispose();
    _timeline.dispose();
    super.dispose();
  }

  // The active object when the timeline last read its keys.
  int _lastActiveObject = 0;

  /// Each object has its own keys, so when another object becomes the
  /// active one the timeline re-reads them (without moving the playhead).
  void _onStudioChanged() {
    final active = _studio.objectList.$2;
    if (active == _lastActiveObject) return;
    _lastActiveObject = active;
    _timeline.reloadKeys();
  }

  /// The window-wide shortcuts. Each one runs the same command as its menu
  /// entry.
  bool _handleKey(KeyEvent event) {
// Space on its own plays and pauses the animation (Ctrl+Space, below,
    // maximizes the viewport).
    if (event is KeyDownEvent &&
        event.logicalKey == LogicalKeyboardKey.space &&
        !HardwareKeyboard.instance.isControlPressed &&
        !isTyping()) {
      _actions.togglePlay(context);
      return true;
    }
    if (!HardwareKeyboard.instance.isControlPressed) return false;
    final key = event.logicalKey;
        if (key == LogicalKeyboardKey.keyZ &&
        (event is KeyDownEvent || event is KeyRepeatEvent) &&
        !isTyping()) {
      _actions.undoBone(context);
      return true;
    }
    if (event is! KeyDownEvent) return false;
    if (key == LogicalKeyboardKey.keyS) {
      _actions.save(context);
      return true;
    }
    if (key == LogicalKeyboardKey.keyO) {
      _actions.load(context);
      return true;
    }
    if (key == LogicalKeyboardKey.space) {
      _actions.toggleMaximize();
      return true;
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          StudioTopBar(actions: _actions),
          Expanded(
            child: ListenableBuilder(
              listenable: _studio,
              builder: (context, _) {
                final viewport = ViewportPane(
                  key: _viewportKey,
                  actions: _actions,
                );
                if (_studio.viewportMaximized) {
                  return viewport;
                }
                return Row(
                  children: [
                    ToolColumn(studio: _studio),
                    Expanded(
                      child: Column(
                        children: [

                          Expanded(child: viewport),
                           ShapesTray(studio: _studio),
                          TimelinePane(actions: _actions),                        ],
                      ),
                    ),
                    SidePanel(actions: _actions),
                  ],
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}
