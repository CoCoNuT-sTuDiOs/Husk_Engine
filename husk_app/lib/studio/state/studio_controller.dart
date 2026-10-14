import 'package:flutter/foundation.dart';
import 'package:husk_app/src/rust/api/simple.dart' as rust;

/// What a click or drag in the viewport does.
enum StudioTool {
  /// Tap a joint to select it, drag a joint to pose it, drag elsewhere to orbit.
  select,

  /// Tap the model to place a bone.
  placeBones,

  /// Drag anywhere to move the whole character.
  move,
}

/// Studio's interface state: what is selected, which tool is active, and so
/// on, plus the skeleton actions the panels trigger. The Rust core stays the
/// source of truth for the scene itself. Fields are added slice by slice.
class StudioController extends ChangeNotifier {
  StudioController() {
    // A model that already has a rig is for posing; a bare one is for
    // building a skeleton.
    _tool = rust.boneCount() == BigInt.zero
        ? StudioTool.placeBones
        : StudioTool.select;
  }

  BigInt? _selectedBone;
  bool _viewportMaximized = false;
  StudioTool _tool = StudioTool.select;
  bool _ikMode = false;

BigInt? get selectedBone => _selectedBone;

  /// How many bones the skeleton has.
  int get boneCount => rust.boneCount().toInt();

  /// The parent of [bone], or null for a root (or an unknown bone).
  BigInt? parentOf(BigInt bone) {
    final parents = rust.boneParents();
    final index = bone.toInt();
    return index < parents.length ? parents[index] : null;
  }

  /// True once skin weights have been computed or loaded.
  bool get hasWeights =>
      rust.getVertexDominantBone(vertexIndex: BigInt.zero) != null;
  /// True while the viewport fills the window and the other panes are hidden.
  bool get viewportMaximized => _viewportMaximized;

  StudioTool get tool => _tool;

  /// When on, dragging the end of a limb bends the joints above it.
  bool get ikMode => _ikMode;

  void selectBone(BigInt? bone) {
    if (_selectedBone == bone) return;
    _selectedBone = bone;
    notifyListeners();
  }

  void toggleViewportMaximized() {
    _viewportMaximized = !_viewportMaximized;
    notifyListeners();
  }

  void setTool(StudioTool tool) {
    if (_tool == tool) return;
    _tool = tool;
    notifyListeners();
  }

  void toggleIk() {
    _ikMode = !_ikMode;
    notifyListeners();
  }

  /// Places a bone where the click lands on the model, with the selected
  /// joint as its parent. Returns a message when the click missed.
  String? placeBoneAt(double x, double y) {
    // Bones are placed against the rest-pose mesh, so go back to rest first.
    rust.resetPose();
    final result = rust.placeBone(x: x, y: y, parent: _selectedBone);
    if (result == null) return 'Bone placement: the click missed the mesh';
    final (index, _, _, _) = result;
    _selectedBone = index;
    notifyListeners();
    return null;
  }

  /// Adds a built-in shape ("cube", "plane", "capsule" or "torus") at
  /// render-pixel position (x, y). Returns a message when it could not be
  /// placed.
  String? addShape(String name, double x, double y) {
    final index = rust.addShape(name: name, x: x, y: y);
    if (index == null) return 'Could not add the $name here';
    notifyListeners();
    return null;
  }

  /// Makes the object under render pixel (x, y) the active one, so the
  /// bone and animation tools act on it. Returns its index, or null when
  /// the click hit no object.
  BigInt? selectObjectAt(double x, double y) {
    final index = rust.selectObjectAt(x: x, y: y);
    if (index == null) return null;
    // A joint selected on the previous object means nothing on this one.
    _selectedBone = null;
    notifyListeners();
    return index;
  }

  /// The scene's object names, in order, and the index of the active one.
  /// The Rust core owns the list; this only reads it.
  (List<String>, int) get objectList {
    final (names, active) = rust.sceneObjects();
    return (names, active.toInt());
  }

  /// Makes the object at [index] the active one.
  void selectObject(int index) {
    if (!rust.selectObject(index: BigInt.from(index))) return;
    // A joint selected on the previous object means nothing on this one.
    _selectedBone = null;
    notifyListeners();
  }

  /// Deletes the object at [index] (see [removeActiveObject]). Returns a
  /// message.
  String removeObject(int index) {
    selectObject(index);
    return removeActiveObject();
  }

  /// Deletes the active object. The loaded model itself (object 0) cannot
  /// be deleted. Returns a message.
  String removeActiveObject() {
    final index = rust.removeActiveObject();
    if (index == null) {
      return 'Select a shape first (the loaded model cannot be deleted)';
    }
    // The loaded model is the active object again; a joint selected on the
    // deleted object means nothing now.
    _selectedBone = null;
    notifyListeners();
    return 'Deleted object $index';
  }

  /// Removes the last placed bone and selects its parent. Returns a message
  /// when there is nothing to undo.
  String? undoLastBone() {
    final (removed, parent) = rust.undoLastBone();
    if (!removed) return 'Nothing to undo';
    _selectedBone = parent;
    notifyListeners();
    return null;
  }

  /// Computes skin weights for the current skeleton. Returns a message.
  String computeWeights() {
    final timer = Stopwatch()..start();
    final success = rust.computeWeights();
    timer.stop();
    // With weights computed the model is for posing, not building.
    if (success) _tool = StudioTool.select;
    notifyListeners();
    return success
        ? 'Weights computed for ${rust.boneCount()} bones '
            'in ${timer.elapsedMilliseconds} ms'
        : 'Weight computation failed (no bones placed yet?)';
  }

  void resetPose() {
    rust.resetPose();
    notifyListeners();
  }

  /// Clears the skeleton, weights and keys. The timeline controller must
  /// refresh afterwards.
  void resetSkeleton() {
    rust.resetSkeleton();
    _selectedBone = null;
    _tool = StudioTool.placeBones;
    notifyListeners();
  }
}