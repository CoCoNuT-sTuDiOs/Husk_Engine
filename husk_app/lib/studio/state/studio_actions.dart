import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/timeline_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/messages.dart';
import 'package:file_selector/file_selector.dart';
import 'package:husk_app/ui/shortcuts_manual.dart';
import 'package:husk_app/studio/state/keymap.dart';

/// The commands a user can trigger from menus, panels, headers and
/// shortcuts. Each one lives here once, so every place that offers it
/// behaves the same.
class StudioActions {
  StudioActions({
    required this.studio,
    required this.timeline,
    required this.viewport,
    this.showHomeScreen,
    this.showStudioScreen,
  });

  final StudioController studio;
  final TimelineController timeline;
  final ViewportController viewport;

  /// Shows the Home screen over the studio (the scene stays as it is).
  final VoidCallback? showHomeScreen;

  /// Hides the Home screen so the studio shows.
  final VoidCallback? showStudioScreen;
  // File

/// Saves to the project's file, or asks where when it has none yet.
  Future<void> save(BuildContext context) async {
    final path = studio.projectPath;
    if (path == null) return saveAs(context);
    showMessage(context, studio.saveProject(path));
  }

  /// Always asks where to save.
  Future<void> saveAs(BuildContext context) async {
    final location = await getSaveLocation(
      acceptedTypeGroups: const [
        XTypeGroup(label: 'Husk projects', extensions: ['husk']),
      ],
      suggestedName: 'untitled.husk', 
    );
    if (location == null) return;
    if (!context.mounted) return;
    var path = location.path;
    if (!path.toLowerCase().endsWith('.husk')) path = '$path.husk';
    showMessage(context, studio.saveProject(path));
  }

Future<void> load(BuildContext context) async {
    final file = await openFile(
      acceptedTypeGroups: const [
        XTypeGroup(label: 'Husk projects', extensions: ['husk']),
      ],
    );
    if (file == null) return;
    if (!context.mounted) return;
    if (!await confirmDiscard(context)) return;
    if (!context.mounted) return;
    final message = studio.openProject(file.path);
    timeline.refresh();
    studio.selectBone(null);
    studio.setTool(StudioTool.select);
    if (studio.projectPath == file.path) showStudioScreen?.call();
    showMessage(context, message);
  }

/// At start-up, offers to recover the work an earlier session left behind
  /// (after a crash, a power cut or closing without saving).
  Future<void> offerRecovery(BuildContext context) async {
    final info = studio.recoveryInfo;
    if (info == null) return;
    final recover = await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Recover unsaved work?'),
        content: Text(
          'Husk found a recovery copy from ${_ago(info.secondsAgo.toInt())} '
          'ago, left by a crash or by closing without saving.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Discard'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Recover'),
          ),
        ],
      ),
    );
    if (!context.mounted) return;
    if (recover == true) {
      showMessage(context, studio.recoverProject(info.path));
      showStudioScreen?.call();
      timeline.refresh();
    } else {
      studio.discardRecovery();
    }
  }

  /// How long ago, in words: "moments", "5 minutes", "3 hours", "2 days".
  String _ago(int seconds) {
    if (seconds < 60) return 'moments';
    final minutes = seconds ~/ 60;
    if (minutes < 60) return '$minutes ${minutes == 1 ? 'minute' : 'minutes'}';
    final hours = minutes ~/ 60;
    if (hours < 24) return '$hours ${hours == 1 ? 'hour' : 'hours'}';
    final days = hours ~/ 24;
    return '$days ${days == 1 ? 'day' : 'days'}';
  }

/// Goes to the Home screen. The scene stays as it is.
  void goHome() => showHomeScreen?.call();

  /// Before the scene is replaced: if it has changes that are not in a saved
  /// project, asks whether to save them. True when it is fine to go on.
  Future<bool> confirmDiscard(BuildContext context) async {
    if (!studio.hasUnsavedChanges) return true;
    final choice = await showDialog<String>(
      context: context,
      barrierDismissible: false,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Save your changes?'),
        content: const Text(
          'The scene has changes that are not in a saved project.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop('cancel'),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop('discard'),
            child: const Text(
              "Don't save",
              style: TextStyle(color: HuskColors.error),
            ),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop('save'),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    if (choice == 'discard') {
      // The work is being thrown away on purpose, so it is not offered back
      // as a recovery copy at the next start.
      studio.discardRecovery();
      return true;
    }
    if (choice != 'save') return false;
    if (!context.mounted) return false;
    await save(context);
    // Saving may have been cancelled in the file dialog.
    return !studio.hasUnsavedChanges;
  }

  /// Starts an empty project that will be saved at [path] in Husk's folder.
  Future<void> startNewProject(BuildContext context, String path) async {
    if (!await confirmDiscard(context)) return;
    studio.newProject(path);
    timeline.refresh();
    showStudioScreen?.call();
  }

  /// File > New project: starts an empty project in Husk's folder, or says
  /// that both slots are used.
  void newProjectFromMenu(BuildContext context) {
    final path = studio.newProjectPath();
    if (path == null) {
      showMessage(
        context,
        'Both project slots are used. Open Home and delete a project to make room.',
      );
      return;
    }
    startNewProject(context, path);
  }

  /// Opens the project at [path] (from the Home screen).
  Future<void> openProjectAt(BuildContext context, String path) async {
    if (!await confirmDiscard(context)) return;
    final message = studio.openProject(path);
    if (studio.projectPath == path) {
      timeline.refresh();
      studio.selectBone(null);
      studio.setTool(StudioTool.select);
      showStudioScreen?.call();
    }
    if (context.mounted) showMessage(context, message);
  }

  /// Asks for a glTF or GLB file and imports it as a new object.
  Future<void> importModel(BuildContext context) async {
    final file = await openFile(
      acceptedTypeGroups: const [
        XTypeGroup(label: 'glTF models', extensions: ['glb', 'gltf']),
      ],
    );
    if (file == null) return;
    if (!context.mounted) return;
    showMessage(context, studio.importModel(file.path));
  }

  // Edit
void undo(BuildContext context) {
    final message = studio.undoEdit();
    timeline.reloadKeys();
    if (message != null) showMessage(context, message);
  }

  void redo(BuildContext context) {
    final message = studio.redoEdit();
    timeline.reloadKeys();
    if (message != null) showMessage(context, message);
  }
void duplicate(BuildContext context) =>
      showMessage(context, studio.duplicateActive());

  /// Duplicates the object at [index], which becomes the active one.
  void duplicateObjectAt(BuildContext context, int index) {
    studio.selectObject(index);
    duplicate(context);
  }

  void copy(BuildContext context) => showMessage(context, studio.copyActive());

  void paste(BuildContext context) => showMessage(context, studio.paste());

  void toggleCameraView(BuildContext context) =>
      showMessage(context, studio.toggleCameraView());

  void addCamera(BuildContext context) =>
        showMessage(context, studio.addCamera());

void addRig(BuildContext context) => showMessage(context, studio.addRig());

  void addLight(BuildContext context, String kind) =>
      showMessage(context, studio.addLight(kind));
  void deleteObject(BuildContext context) =>
      showMessage(context, studio.removeActiveObject());

  void deleteObjectAt(BuildContext context, int index) =>
      showMessage(context, studio.removeObject(index));
void setBoneParent(BuildContext context, int child, int parent, int bone) =>
      showMessage(context, studio.setBoneParent(child, parent, bone));

  void setParent(BuildContext context, int child, int? parent) =>
      showMessage(context, studio.setParent(child, parent));

    void resetPose() => studio.resetPose();

  /// Puts the active object in the named mode (the mode menu).
  void setMode(BuildContext context, String name) =>
      showMessage(context, studio.setMode(name));

  /// Ctrl+Tab: between Object mode and Pose mode.
  void toggleMode(BuildContext context) =>
      showMessage(context, studio.toggleObjectPose());

  void setLocked(BuildContext context, int index, bool locked) =>
      showMessage(context, studio.setLocked(index, locked));

  void lockOthers(BuildContext context) =>
      showMessage(context, studio.lockOthers());

  void unlockAll(BuildContext context) =>
      showMessage(context, studio.unlockAll());

  /// Clears the skeleton, weights and keys, after asking.
  Future<void> resetSkeleton(BuildContext context) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Clear the skeleton?'),
        content: const Text(
          'This removes every bone, the skin weights and all keys. '
          'It cannot be undone.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: HuskColors.error,
              foregroundColor: Colors.white,
            ),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Clear'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    studio.resetSkeleton();
    timeline.refresh();
    if (context.mounted) showMessage(context, 'Skeleton cleared');
  }

  // Rig

  void computeWeights(BuildContext context) =>
      showMessage(context, studio.computeWeights());

  // View

void toggleMaximize() => studio.toggleViewportMaximized();

  void toggleToolPanel() => studio.toggleToolsCollapsed();

void toggleSidePanel() => studio.toggleSidePanelCollapsed();

  void setLowQuality() => studio.setQuality(ViewportQuality.low);

  void setNormalQuality() => studio.setQuality(ViewportQuality.normal);
  // Help

  bool _shortcutsOpen = false;

  bool get shortcutsOpen => _shortcutsOpen;

  /// Opens the shortcuts manual (Help > Keyboard Shortcuts, Ctrl+/).
  Future<void> showShortcuts(BuildContext context) async {
    if (_shortcutsOpen) return;
    _shortcutsOpen = true;
    await showDialog<void>(
      context: context,
      builder: (_) => const ShortcutsDialog(),
    );
    _shortcutsOpen = false;
  }

  /// Turns the viewport to front, back, right, left, top or bottom.
  void setStandardView(String name) {
    studio.leaveCameraView();
    viewport.standardView(name);
  }

  /// Moves the viewport to see every object, as Blender's Frame All does.
  void frameAll(BuildContext context) {
    studio.leaveCameraView();
    if (!viewport.frameAll()) showMessage(context, 'Nothing to frame');
  }

  /// Moves the viewport to see the active object, as Blender's Frame
  /// Selected does.
  void frameSelected(BuildContext context) {
    studio.leaveCameraView();
    if (!viewport.frameSelected()) {
      showMessage(context, 'The active object has no shape to frame');
    }
  }

  /// Moves the view away, as the - key does.
  void zoomOut() {
    studio.leaveCameraView();
    viewport.zoom(1.08);
  }

  /// Brings the view closer, as the = key does.
  void zoomIn() {
    studio.leaveCameraView();
    viewport.zoom(1 / 1.08);
  }

  /// Switches the viewport between perspective and orthographic.
  void toggleOrthographic() {
    studio.leaveCameraView();
    viewport.toggleOrthographic();
  }


  // Keys

  /// Runs the command a key is bound to (see keymap.dart). Each command
  /// calls the same method its menu entry calls.
  void runCommand(BuildContext context, HuskCommand command) {
    switch (command) {
      case HuskCommand.save:
        save(context);
      case HuskCommand.saveAs:
        saveAs(context);
      case HuskCommand.load:
        load(context);
        case HuskCommand.undo:
        undo(context);
      case HuskCommand.redo:
        redo(context);
      case HuskCommand.duplicate:
        duplicate(context);
      case HuskCommand.copy:
        copy(context);
      case HuskCommand.paste:
        paste(context);
      case HuskCommand.deleteObject:
        deleteObject(context);
      case HuskCommand.zoomIn:
        zoomIn();
      case HuskCommand.zoomOut:
        zoomOut();
      case HuskCommand.frameAll:
        frameAll(context);
      case HuskCommand.frameSelected:
        frameSelected(context);
      case HuskCommand.viewFront:
        setStandardView('front');
      case HuskCommand.viewBack:
        setStandardView('back');
      case HuskCommand.viewRight:
        setStandardView('right');
      case HuskCommand.viewLeft:
        setStandardView('left');
      case HuskCommand.viewTop:
        setStandardView('top');
      case HuskCommand.viewBottom:
        setStandardView('bottom');
      case HuskCommand.toggleOrthographic:
        toggleOrthographic();
      case HuskCommand.togglePlay:
        togglePlay(context);
      case HuskCommand.toggleMaximize:
        toggleMaximize();
          case HuskCommand.showShortcuts:
        showShortcuts(context);
      case HuskCommand.toggleToolPanel:
        toggleToolPanel();
      case HuskCommand.toggleSidePanel:
        toggleSidePanel();
      case HuskCommand.toggleMode:
      toggleMode(context);
      case HuskCommand.brushSmaller:
        studio.nudgeBrushRadius(1 / 1.15);
      case HuskCommand.brushBigger:
        studio.nudgeBrushRadius(1.15);
      case HuskCommand.toolMoveObject:
         showMessage(context, studio.setTool(StudioTool.moveObject));
      case HuskCommand.toolRotateObject:
          showMessage(context, studio.setTool(StudioTool.rotateObject));
      case HuskCommand.toolScaleObject:
        studio.setTool(StudioTool.scaleObject);
    }
  }

  // Timeline

  void togglePlay(BuildContext context) =>
      showMessage(context, timeline.togglePlay());

  void addKey(BuildContext context) => showMessage(context, timeline.addKey());

  void deleteKey(BuildContext context) =>
      showMessage(context, timeline.deleteKey());

  void toggleRepeat(BuildContext context) =>
      showMessage(context, timeline.toggleRepeat());
}
