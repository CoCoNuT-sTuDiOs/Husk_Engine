import 'dart:typed_data';

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

  /// Drag an arrow on the active object to move it along that axis.
  moveObject,

  /// Drag a ring around the active object to turn it about that axis.
  rotateObject,

  /// Drag a handle on the active object to scale it along that axis, or
  /// the centre box to scale all three axes together.
  scaleObject,
}

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

  /// The built-in shapes' names, in the order the tray shows them. The
  /// Rust core owns the list.
  List<String> get shapeNames => rust.shapeNames();

  Future<Uint8List?> shapePreview(String name, int size) =>
      rust.shapePreview(name: name, size: size);


  String? addShape(String name, double x, double y) {
    final index = rust.addShape(name: name, x: x, y: y);
    if (index == null) return 'Could not add the $name here';
    notifyListeners();
    return null;
  }

  BigInt? selectObjectAt(double x, double y) {
    final index = rust.selectObjectAt(x: x, y: y);
    if (index == null) return null;
    // A joint selected on the previous object means nothing on this one.
    _selectedBone = null;
    notifyListeners();
    return index;
  }

  /// True while the viewport looks through the scene camera.
  bool get cameraView => rust.cameraViewActive();

  /// Starts or stops looking through the scene camera. Returns a message
  /// when there is no camera to look through.
  String? toggleCameraView() {
    final wanted = !rust.cameraViewActive();
    final active = rust.setCameraView(on_: wanted);
    notifyListeners();
    if (wanted && !active) return 'Add a camera first (Add > Camera)';
    return null;
  }

  /// Leaves camera view. Orbiting or zooming the viewport does this, since
  /// they move the viewport's own camera.
  void leaveCameraView() {
    if (!rust.cameraViewActive()) return;
    rust.setCameraView(on_: false);
    notifyListeners();
  }

  /// The finished video's width, height (pixels) and frame rate.
  (int, int, int) get outputSettings => rust.outputSettings();

  /// Sets the finished video's width, height and frame rate. The engine
  /// holds them inside their ranges and makes the sides even.
  void setOutputSettings(int width, int height, int fps) {
    rust.setOutputSettings(width: width, height: height, fps: fps);
    notifyListeners();
  }

  /// Adds a scene camera where the viewport is looking from. Returns a
  /// message when it could not be added.
  String? addCamera() {
    final index = rust.addCamera();
    if (index == null) return 'Could not add a camera';
    notifyListeners();
    return null;
  }

  /// Adds a light ('sun' or 'point') and makes it the active object, so the
  /// Properties panel shows it. Returns a message when it could not be added.
  String? addLight(String kind) {
    final index = rust.addLight(kind: kind);
    if (index == null) return 'Could not add a $kind light';
    selectObject(index.toInt());
    notifyListeners();
    return null;
  }

  /// The active object's light if it is a light, otherwise null. The Rust
  /// core owns it; this only reads it.
  rust.ObjectLight? get activeLight => rust.activeLight();

  /// Sets the active light's colour and intensity. Returns false if the
  /// engine refused it (the active object is not a light).
  bool setActiveLight((double, double, double) color, double intensity) {
    final applied = rust.setActiveLight(color: color, intensity: intensity);
    if (applied) notifyListeners();
    return applied;
  }

  /// The active object's field of view in degrees if it is a camera,
  /// otherwise null.
  double? get activeCameraFov => rust.activeCameraFov();

  /// Sets the active camera's field of view. Returns false if the engine
  /// refused it (the active object is not a camera).
  bool setActiveCameraFov(double degrees) {
    final applied = rust.setActiveCameraFov(degrees: degrees);
    if (applied) notifyListeners();
    return applied;
  }

  /// The scene's world: background colour and studio light. The Rust core
  /// owns it; this only reads it.
  rust.WorldSettings get world => rust.worldSettings();

  /// Applies [settings] to the world. Returns false if the engine refused
  /// it (a value that is not a finite number).
  bool setWorld(rust.WorldSettings settings) {
    final applied = rust.setWorldSettings(settings: settings);
    if (applied) notifyListeners();
    return applied;
  }

  /// The active object's material. The Rust core owns it; this only
  /// it.
  rust.ObjectMaterial get activeMaterial => rust.activeObjectMaterial();

  /// Applies [material] to the active object. Returns false if the engine
  /// refused it (a value that is not a finite number).
  bool setActiveMaterial(rust.ObjectMaterial material) {
    final applied = rust.setActiveObjectMaterial(material: material);
    if (applied) notifyListeners();
    return applied;
  }

  /// The active object's own X, Y and Z axes as directions in the scene.
  List<(double, double, double)> get activeLocalAxes {
    final (x, y, z) = rust.activeObjectAxes();
    return [x, y, z];
  }

  /// The active object's transform. The Rust core owns it; this only
  /// reads it.
  rust.ObjectTransform get activeTransform => rust.activeObjectTransform();
  bool setActiveTransform(rust.ObjectTransform transform) {
    final applied = rust.setActiveObjectTransform(transform: transform);
    if (applied) notifyListeners();
    return applied;
  }

  /// Turns the active object to [degrees] (X, Y, Z), keeping its position
  /// and scale.
  bool setActiveRotation((double, double, double) degrees) {
    final current = rust.activeObjectTransform();
    return setActiveTransform(
      rust.ObjectTransform(
        position: current.position,
        rotationDegrees: degrees,
        scale: current.scale,
      ),
    );
  }


  /// Sets the active object's scale to [scale] (X, Y, Z), keeping its
  /// position and rotation.
  bool setActiveScale((double, double, double) scale) {
    final current = rust.activeObjectTransform();
    return setActiveTransform(
      rust.ObjectTransform(
        position: current.position,
        rotationDegrees: current.rotationDegrees,
        scale: scale,
      ),
    );
  }

  /// Moves the active object to [position], keeping its rotation and scale.
  bool setActivePosition((double, double, double) position) {
    final current = rust.activeObjectTransform();
    return setActiveTransform(
      rust.ObjectTransform(
        position: position,
        rotationDegrees: current.rotationDegrees,
        scale: current.scale,
      ),
    );
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