import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/studio/editors/timeline/timeline_pane.dart';
import 'package:husk_app/studio/editors/viewport/viewport_pane.dart';
import 'package:husk_app/studio/shell/side_panel.dart';
import 'package:husk_app/studio/shell/studio_top_bar.dart';
import 'package:husk_app/studio/state/studio_actions.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/timeline_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/home/about_husk.dart';
import 'package:husk_app/home/home_screen.dart';
import 'package:husk_app/ui/typing.dart';
import 'package:husk_app/studio/state/keymap.dart';

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
    showHomeScreen: () => setState(() => _showHome = true),
    showStudioScreen: () => setState(() => _showHome = false),
  );

  // Keeps the viewport's state (texture, size) when it moves between the
  // normal layout and the maximized one.
    final GlobalKey _viewportKey = GlobalKey();

  // Whether the Home screen covers the studio. Husk opens on Home, and the
  // studio stays alive underneath so nothing is lost when you go back.
  bool _showHome = true;

  // Writes a recovery copy of the scene every couple of minutes when it has
  // changed.
  Timer? _autosaveTimer;

  @override
  void initState() {
    super.initState();
    HardwareKeyboard.instance.addHandler(_handleKey);
    _autosaveTimer = Timer.periodic(
      const Duration(minutes: 2),
      (_) => _studio.autosave(),
    );
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _actions.offerRecovery(context);
    });
    _lastActiveObject = _studio.objectList.$2;
    _studio.addListener(_onStudioChanged);
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_handleKey);
    _autosaveTimer?.cancel();
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
    // While the shortcuts manual is open it has the keys: Delete, Space and
    // the number keys must not act on the scene behind it.
  if (_showHome || _actions.shortcutsOpen) return false;
    if (event is! KeyDownEvent && event is! KeyRepeatEvent) return false;
    final binding = bindingForKey(
      event.logicalKey,
      ctrl: HardwareKeyboard.instance.isControlPressed,
      shift: HardwareKeyboard.instance.isShiftPressed,
      isRepeat: event is KeyRepeatEvent,
    );
    if (binding == null) return false;
    // A text box has the keys, except for the rows the table marks as
    // working while typing.
    if (isTyping() && !binding.whileTyping) return false;
    _actions.runCommand(context, binding.command);
    return true;
  }

  @override
Widget build(BuildContext context) {
    return Stack(
      children: [
        Offstage(offstage: _showHome, child: _buildStudio(context)),
        if (_showHome)
          HomeScreen(
            onNewProject: (path) => _actions.startNewProject(context, path),
            onOpenProject: (path) => _actions.openProjectAt(context, path),
            onOpenFile: () => _actions.load(context),
            onShowShortcuts: () => _actions.showShortcuts(context),
            onShowAbout: () => showAboutHusk(context),
            onBackToScene: () => setState(() => _showHome = false),
          ),
      ],
    );
  }

  Widget _buildStudio(BuildContext context) {
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
                    Expanded(
                      child: Column(
                        children: [
                          Expanded(child: viewport),
                          TimelinePane(actions: _actions),
                        ],
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
