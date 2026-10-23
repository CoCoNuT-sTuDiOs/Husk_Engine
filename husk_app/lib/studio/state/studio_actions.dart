import 'package:flutter/material.dart';
import 'package:husk_app/studio/state/studio_controller.dart';
import 'package:husk_app/studio/state/timeline_controller.dart';
import 'package:husk_app/studio/state/viewport_controller.dart';
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/messages.dart';

/// The commands a user can trigger from menus, panels, headers and
/// shortcuts. Each one lives here once, so every place that offers it
/// behaves the same.
class StudioActions {
  StudioActions({
    required this.studio,
    required this.timeline,
    required this.viewport,
  });

  final StudioController studio;
  final TimelineController timeline;
  final ViewportController viewport;

  // File

  void save(BuildContext context) => showMessage(context, timeline.save());

  void load(BuildContext context) {
    final message = timeline.load();
    studio.selectBone(null);
    studio.setTool(StudioTool.select);
    showMessage(context, message);
  }

  // Edit

  void undoBone(BuildContext context) =>
      showMessage(context, studio.undoLastBone());

  void toggleCameraView(BuildContext context) =>
      showMessage(context, studio.toggleCameraView());

  void addCamera(BuildContext context) =>
        showMessage(context, studio.addCamera());

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

  // Timeline

  void togglePlay(BuildContext context) =>
      showMessage(context, timeline.togglePlay());

  void addKey(BuildContext context) => showMessage(context, timeline.addKey());

  void deleteKey(BuildContext context) =>
      showMessage(context, timeline.deleteKey());

  void toggleRepeat(BuildContext context) =>
      showMessage(context, timeline.toggleRepeat());
}
