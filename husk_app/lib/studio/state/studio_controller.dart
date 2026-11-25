
import 'package:flutter/foundation.dart';
import 'package:husk_app/src/rust/api/simple.dart' as rust;
import 'package:shared_preferences/shared_preferences.dart';

/// What a click or drag in the viewport does.
enum StudioTool {
  select,
  placeBones,
  move,
  moveObject,
  rotateObject,

  /// Drag a handle on the active object to scale it along that axis, or
  /// the centre box to scale all three axes together.
  scaleObject,
}

/// How much the viewport draws. Low renders at half resolution for slow
/// laptops and Normal at full resolution. Shadows and subdivision levels join
/// this setting when they exist.
enum ViewportQuality {
  low(renderScale: 0.5, maxSide: 1280),
  normal(renderScale: 1.0, maxSide: 1920);

  const ViewportQuality({required this.renderScale, required this.maxSide});

  /// The share of the window's real pixels the viewport draws.
  final double renderScale;

  /// The most pixels the viewport draws along its longer side.
  final double maxSide;
}

class StudioController extends ChangeNotifier {
  StudioController() {
    _loadHints();
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
  bool _toolsCollapsed = false;
  bool _sidePanelCollapsed = true;
  bool _snapToSurface = false;

  // The first-time hint cards the user has already closed.
final Set<String> _seenHints = <String>{};
  bool _hintsLoaded = false;

  // The weight brush (Weight Paint mode). The starting values are Blender's.
  double _brushWeight = 1.0;
  double _brushStrength = 1.0;
  double _brushRadius = 50;
  String _brushBlend = 'mix';
  bool _brushAccumulate = false;
 String? _projectPath;
  ViewportQuality _quality = ViewportQuality.normal;
 BigInt? get selectedBone => _selectedBone;


  /// Tells the viewport to show the weights of the selected bone while the
  /// active object is in Weight Paint mode, and its normal look otherwise.
  void _syncWeightView() {
    final showing = rust.activeMode() == 'weight_paint';
    rust.setWeightView(bone: showing ? (_selectedBone ?? BigInt.zero) : null);
  }

  /// Every change is also a chance for the mode or the selected bone to have
  /// changed, so the weight colours follow them.
  @override
  void notifyListeners() {
    _syncWeightView();
    super.notifyListeners();
  }

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
/// Puts the active object on the surface under the cursor, turned to lie on
  /// it. [x] and [y] are render pixels. False over empty space.
  bool snapActiveToSurface(double x, double y) {
    final applied = rust.snapActiveToSurface(x: x, y: y);
    if (applied) notifyListeners();
    return applied;
  }

  /// When on, shapes dropped from the tray and the active object dragged with
  /// the Move object tool stick to the surface under the cursor.
  bool get snapToSurface => _snapToSurface;

  void toggleSnapToSurface() {
    _snapToSurface = !_snapToSurface;
    notifyListeners();
  }

/// How much the viewport draws (see [ViewportQuality]).
  ViewportQuality get quality => _quality;

  /// Whether the viewport is on the Low setting.
  bool get lowQuality => _quality == ViewportQuality.low;

  void setQuality(ViewportQuality quality) {
    if (_quality == quality) return;
    _quality = quality;
    notifyListeners();
  }

  /// True while the left tool strip is folded away to a thin rail.
  bool get toolsCollapsed => _toolsCollapsed;

  /// True while the right Outliner and Properties panel is folded away to a
  /// thin rail.
  bool get sidePanelCollapsed => _sidePanelCollapsed;

  void toggleToolsCollapsed() {
    _toolsCollapsed = !_toolsCollapsed;
    notifyListeners();
  }

  void toggleSidePanelCollapsed() {
    _sidePanelCollapsed = !_sidePanelCollapsed;
    notifyListeners();
  }

  /// True while the viewport fills the window and the other panes are hidden.
  bool get viewportMaximized => _viewportMaximized;

    /// The tools each mode offers. Place bones now lives in Edit Mode, which offers Select and
  /// Place bones. Object mode keeps the object tools.
  static List<StudioTool> toolsInMode(String mode) {
    switch (mode) {
      case 'pose':
        return const [StudioTool.select, StudioTool.move];
      case 'weight_paint':
                return const [StudioTool.select];
      case 'edit':
        return const [StudioTool.select, StudioTool.placeBones];
      default:
        return const [
          StudioTool.select,
          StudioTool.moveObject,
          StudioTool.rotateObject,
          StudioTool.scaleObject,
        ];
    }
  }

  /// The tool in use. A tool the current mode does not offer is not in use:
  /// Select stands in, so the viewport, the header and the keys agree.
  StudioTool get tool =>
      toolsInMode(mode).contains(_tool) ? _tool : StudioTool.select;

  /// The mode the active object is in: 'object', 'pose' or 'weight_paint'.
  /// The Rust core owns it; this only reads it.
  String get mode => rust.activeMode();

  /// The modes the active object allows, in menu order.
  List<String> get availableModes => rust.availableModes();

  /// Puts the active object in the named mode. Returns a message when the
  /// object does not allow it.
  String? setMode(String name) {
    if (rust.setMode(name: name)) {
      _grabbedBone = null;
      // Weight Paint always has a bone to show: the first one until you pick.
    if (name == 'weight_paint') _selectedBone ??= BigInt.zero;
      // Edit Mode works on the rest pose, so a pose the character was left
      // in goes back to rest.
      if (name == 'edit') {
        rust.resetPose();
        rust.refreshModels();
      }
      notifyListeners();
      return null;
    }
    switch (name) {
      case 'pose':
        return 'Pose Mode needs a rig, or a mesh that has bones. '
            'Use Edit Mode and Place bones first.';
                case 'edit':
        return 'Edit Mode needs a rig or a shape. Add a rig, or drop a shape '
            'from the tray, first.';
      case 'weight_paint':
        return 'Weight Paint needs a mesh that has bones. '
            'Use Edit Mode and Place bones first.';  
      default:
        return 'This object cannot go into that mode';
    }
  }

  /// Ctrl+Tab: between Object mode and Pose mode, as Blender does on an
  /// armature.
  String? toggleObjectPose() => setMode(mode == 'pose' ? 'object' : 'pose');

  /// Tab: between Object mode and Edit mode, as Blender does.
  String? toggleObjectEdit() => setMode(mode == 'edit' ? 'object' : 'edit');

/// Whether the first-time hint card named [id] has been closed already.
bool hintSeen(String id) => !_hintsLoaded || _seenHints.contains(id);

/// Reads which hint cards were closed in earlier sessions.
  Future<void> _loadHints() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      _seenHints.addAll(prefs.getStringList('seen_hints') ?? const <String>[]);
    } catch (error) {
      debugPrint('Husk: the saved hints could not be read: $error');
    }
    _hintsLoaded = true;
    notifyListeners();
  }

  Future<void> _saveHints() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setStringList('seen_hints', _seenHints.toList());
    } catch (error) {
      debugPrint('Husk: the hints could not be saved: $error');
    }
  }



// The hint cards that are on screen right now, so that one can be marked as
  // seen the moment what it describes is over.
  final Set<String> _hintsOnScreen = <String>{};

  /// Whether the hint card named [id] shows now. [inUse] says whether the thing
  /// it is about is happening. A card shows while that is going on for the first
  /// time, and is marked as seen the moment it ends: it never comes back and it
  /// never needs a click to go away.
  bool hintVisible(String id, bool inUse) {
    if (hintSeen(id)) return false;
    if (inUse) {
      _hintsOnScreen.add(id);
      return true;
    }
    if (_hintsOnScreen.remove(id)) {
      // Marked once this frame is built, since marking tells the listeners.
      Future.microtask(() => markHintSeen(id));
    }
    return false;
  }

  /// Closes the hint card named [id], so it is not shown again.
  void markHintSeen(String id) {
if (!_seenHints.add(id)) return;
    notifyListeners();
    _saveHints();
  }

  /// The weight the brush paints, 0 to 1.
  double get brushWeight => _brushWeight;

  /// How strongly one touch paints, 0 to 1.
  double get brushStrength => _brushStrength;

  /// The brush radius in panel pixels: the size of the circle on screen.
  double get brushRadius => _brushRadius;

  /// How a touch combines with what the vertex has: 'mix', 'add' or 'subtract'.
  String get brushBlend => _brushBlend;

  /// When on, staying in one place keeps building up.
  bool get brushAccumulate => _brushAccumulate;

  void setBrushWeight(double value) {
    _brushWeight = value.clamp(0.0, 1.0).toDouble();
    notifyListeners();
  }

  void setBrushStrength(double value) {
    _brushStrength = value.clamp(0.0, 1.0).toDouble();
    notifyListeners();
  }


/// Makes the brush bigger (a factor above 1) or smaller (below 1), the way
  /// the [ and ] keys do. Only in Weight Paint mode.
  void nudgeBrushRadius(double factor) {
    if (mode != 'weight_paint') return;
    setBrushRadius((_brushRadius * factor).clamp(5.0, 200.0).toDouble());
  }

  void setBrushRadius(double value) {
    _brushRadius = value.clamp(5.0, 300.0).toDouble();
    notifyListeners();
  }

  void setBrushBlend(String blend) {
    _brushBlend = blend;
    notifyListeners();
  }

  void setBrushAccumulate(bool on) {
    _brushAccumulate = on;
    notifyListeners();
  }

  /// Starts a weight paint stroke (mouse down) on the selected bone, which is
  /// bone 0 until one is picked, as the weight colours show. [erase] paints
  /// this stroke as Subtract (Ctrl held), and [smooth] smooths the weights
  /// instead (Shift held). Returns why not when it cannot start.
  String? beginPaintStroke({required bool erase, bool smooth = false}) {
    final (started, reason) = rust.weightPaintBegin(
      bone: _selectedBone ?? BigInt.zero,
            mode: (smooth || _brushBlend == 'smooth')
          ? 'smooth'
          : (erase ? 'subtract' : _brushBlend),
      weight: _brushWeight,
      strength: _brushStrength,
      accumulate: _brushAccumulate,
    );
    return started ? null : reason;
  }

  /// One touch of the brush at render pixel ([x], [y]), reaching
  /// [renderRadius] render pixels. Returns whether any vertex was painted.
  bool paintStep(double x, double y, double renderRadius) =>
      rust.weightPaintStep(x: x, y: y, radiusPixels: renderRadius);

  /// Ends the stroke (mouse up).
  void endPaintStroke() {
    rust.weightPaintEnd();
    notifyListeners();
  }

  /// Whether the active object's skin blends dual quaternions (Preserve
  /// volume). The Rust core owns it; this only reads it.
  bool get preserveVolume => rust.preserveVolume();

  void setPreserveVolume(bool enabled) {
    if (rust.setPreserveVolume(enabled: enabled)) notifyListeners();
  }

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

  /// Chooses a tool. Returns a message when the current mode does not offer it.
  String? setTool(StudioTool tool) {
    if (!toolsInMode(mode).contains(tool)) {
      return 'That tool is not available in this mode. '
          'Ctrl+Tab or the mode menu changes the mode.';
    }
    if (_tool == tool) return null;
    _tool = tool;
    notifyListeners();
    return null;
  }


  void toggleIk() {
    _ikMode = !_ikMode;
    notifyListeners();
  }

  String? placeBoneAt(double x, double y) {
    // Bones are placed against the rest-pose mesh, so go back to rest first.
    rust.resetPose();
    final result = rust.placeBone(x: x, y: y, parent: _selectedBone);
        if (result == null) {
      final (names, active) = objectList;
      final name = active < names.length ? names[active] : 'none';
      return 'The click missed $name. Bones go inside the active object: click the object in the Outliner first';
    }
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
  final index = rust.addShape(name: name, x: x, y: y, snap: _snapToSurface);
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

/// Adds a rig (an object that owns a skeleton and has no mesh to click
  /// inside) at the middle of the view, makes it the active object and
  /// switches to the Place bones tool. Returns a message when it could not
  /// be added.
  String? addRig() {
    final index = rust.addRig();
    if (index == null) return 'Could not add a rig';
    selectObject(index.toInt());
// Bones are placed in Edit Mode.
    setMode('edit');
    setTool(StudioTool.placeBones);
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

  bool setActiveLight((double, double, double) color, double intensity) {
    final applied = rust.setActiveLight(color: color, intensity: intensity);
    if (applied) notifyListeners();
    return applied;
  }

  /// The active object's field of view in degrees if it is a camera,
  /// otherwise null.
  double? get activeCameraFov => rust.activeCameraFov();

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

  List<int?> get objectParents =>
      rust.objectParents().map((parent) => parent?.toInt()).toList();

  /// For each object, in list order, whether it is locked. A locked object
  /// cannot be clicked in the viewport, moved or deleted.
  List<bool> get objectLocks => rust.objectLocks();

  /// Whether the active object is locked.
  bool get activeLocked => rust.activeObjectLocked();

  /// Locks or unlocks the object at [index]. Returns a message.
  String setLocked(int index, bool locked) {
    final applied =
        rust.setObjectLocked(index: BigInt.from(index), locked: locked);
    if (!applied) return 'Could not change that lock';
    notifyListeners();
    return locked ? 'Locked' : 'Unlocked';
  }

  /// Locks everything except the active object. Returns a message.
  String lockOthers() {
    rust.lockAllExceptActive();
    notifyListeners();
    return 'Everything except the active object is locked';
  }

  /// Unlocks every object. Returns a message.
  String unlockAll() {
    rust.unlockAllObjects();
    notifyListeners();
    return 'Everything is unlocked';
  }

  /// Hangs the object at [child] below the one at [parent], or frees it when
  /// [parent] is null. It stays where it is. Returns a message.
  String setParent(int child, int? parent) {
    final applied = rust.setObjectParent(
      child: BigInt.from(child),
      parent: parent == null ? null : BigInt.from(parent),
    );
    if (!applied) return 'Could not parent that object (it would make a loop)';
    notifyListeners();
    return parent == null ? 'Freed from its parent' : 'Parented';
  }

  /// For each object, in list order, the bone of its parent that it follows
  /// through the pose, or null.
  List<int?> get objectBoneParents =>
      rust.objectBoneParents().map((bone) => bone?.toInt()).toList();

  /// Makes the object at [child] follow bone [bone] of the object at
  /// [parent] through the pose. It stays where it is. Returns a message.
  String setBoneParent(int child, int parent, int bone) {
    final applied = rust.setObjectBoneParent(
      child: BigInt.from(child),
      parent: BigInt.from(parent),
      bone: BigInt.from(bone),
    );
    if (!applied) return 'Could not attach that object to the bone';
    notifyListeners();
    return 'Attached to bone $bone';
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
      return 'Nothing deleted: select an unlocked shape (the loaded model cannot be deleted)';
    }
    // The loaded model is the active object again; a joint selected on the
    // deleted object means nothing now.
    _selectedBone = null;
    notifyListeners();
    return 'Deleted object $index';
  }

  /// Imports a glTF or GLB file as a new object and selects it; the viewport
  /// then looks at it. Returns a message.
  String importModel(String path) {
    final (imported, message) = rust.importGlb(path: path);
    if (!imported) return message;
    // Importing moves the viewport's own camera.
    leaveCameraView();
    // A joint selected on the previous object means nothing on this one.
    _selectedBone = null;
    // A rigged model is for posing; a bare one is for building a skeleton.
    _tool = rust.boneCount() == BigInt.zero
        ? StudioTool.placeBones
        : StudioTool.select;
    notifyListeners();
    return message;
  }

  /// Removes the last placed bone and selects its parent. Returns a message
  /// when there is nothing to undo.
  String? undoLastBone() {
final (removed, parent) = rust.undoLastBone();
    rust.refreshModels();
    if (!removed) return 'Nothing to undo';
    _selectedBone = parent;
    notifyListeners();
    return null;
  }

/// Goes back one step in the scene's undo history (kept in the Rust core).
  /// Returns a message when there is nothing to undo.
  String? undoEdit() {
    if (!rust.undoEdit()) return 'Nothing to undo';
    _afterHistoryStep();
    return null;
  }

  /// Goes forward again after an undo. Returns a message when there is
  /// nothing to redo.
  String? redoEdit() {
    if (!rust.redoEdit()) return 'Nothing to redo';
    _afterHistoryStep();
    return null;
  }

  /// What undo and redo leave behind: a joint selected before means nothing
  /// now, and every panel reads the scene again.
void _afterHistoryStep() {
    _grabbedBone = null;
    _selectedBone = null;
    notifyListeners();
  }

/// Duplicates the active object beside itself and selects the copy.
  /// Returns a message.
  String duplicateActive() {
    final index = rust.duplicateActiveObject();
    if (index == null) return 'Nothing to duplicate: select a shape first';
    _afterCopy();
    return 'Duplicated as object $index';
  }

  /// Copies the active object to the clipboard. Returns a message.
  String copyActive() => rust.copyActiveObject()
      ? 'Copied'
      : 'Nothing to copy: select a shape first';

  /// Pastes the clipboard as a new object and selects it. Returns a message.
  String paste() {
    final index = rust.pasteObject();
    if (index == null) return 'Nothing to paste: copy a shape first (Ctrl+C)';
    _afterCopy();
    return 'Pasted as object $index';
  }

  /// A new copy is the active object. A joint selected before means nothing
  /// on it, and the Move object tool is ready to put it where it belongs.
  void _afterCopy() {
    _selectedBone = null;
    if (_tool == StudioTool.select || _tool == StudioTool.placeBones) {
      _tool = StudioTool.moveObject;
    }
    notifyListeners();
  }

/// The file the project was last opened from or saved to, or null for a new
  /// project that has no file yet.
  String? get projectPath => _projectPath;

  /// The name the top bar shows: the project file's name, or Untitled project.
  String get projectName {
    final path = _projectPath;
    if (path == null) return 'Untitled project';
    return path.split(RegExp(r'[\\/]')).last;
  }

  /// Saves the whole scene to [path] and remembers it as the project's file.
  /// Returns a message.
  String saveProject(String path) {
    final (saved, message) = rust.saveScene(path: path);
    if (saved) {
      _projectPath = path;
      notifyListeners();
    }
    return message;
  }

  /// Opens the project file at [path], replacing the whole scene. Returns a
  /// message.
  String openProject(String path) {
    final (opened, message) = rust.loadScene(path: path);
    if (!opened) return message;
    _projectPath = path;
    _selectedBone = null;
    leaveCameraView();
    notifyListeners();
    return message;
  }

/// Writes a recovery copy of the scene if it changed since the last one.
  void autosave() => rust.autosaveIfChanged();

  /// The recovery copy an earlier session left, or null.
  rust.RecoveryInfo? get recoveryInfo => rust.recoveryInfo();

  /// Opens the recovery copy at [path] as an untitled project, so saving asks
  /// where to put it. The copy stays until the work is saved. Returns a
  /// message.
  String recoverProject(String path) {
    final (opened, message) = rust.loadScene(path: path);
    if (!opened) return message;
    _projectPath = null;
    rust.markSceneUnsaved();
    _selectedBone = null;
    leaveCameraView();
    notifyListeners();
    return 'Recovered your unsaved work. Save it with Ctrl+S.';
  }

  void discardRecovery() => rust.discardRecovery();

/// Whether the scene has changes that are not in a saved project file.
  bool get hasUnsavedChanges => rust.hasUnsavedChanges();

  /// A path in Husk's own project folder for a new project, or null when both
  /// project slots are used.
  String? newProjectPath() => rust.homeNewProjectPath();

  /// Starts a fresh, empty project that will be saved at [path].
  void newProject(String path) {
    rust.newScene();
    _projectPath = path;
    _selectedBone = null;
    _tool = StudioTool.select;
    leaveCameraView();
    notifyListeners();
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


  BigInt? _grabbedBone;

  // The joint an extrude grew from, so cancelling goes back to selecting it.
  BigInt? _extrudedFrom;

  /// True while a joint follows the mouse (G, or E after it grew a new joint):
  /// a click puts it down, and Esc or a right click cancels.
  bool get grabbing => _grabbedBone != null;

  /// Edit Mode: starts moving the selected joint with the mouse, or with
  /// [extrude] a new joint grown out of it (Blender's E). The whole grab is one
  /// undo step. Returns a message when it cannot start.
  String? startGrab({required bool extrude}) {
    if (mode != 'edit') return null;
    final bone = _selectedBone;
    if (bone == null) return 'Click a joint dot first, then press E or G';
    if (!rust.grabBegin()) return 'This object is locked';
    if (extrude) {
      final grown = rust.extrudeJoint(parent: bone);
      if (grown == null) {
        rust.grabCancel();
        return 'Could not grow a bone from that joint';
      }
      _extrudedFrom = bone;
      _selectedBone = grown;
      _grabbedBone = grown;
    } else {
      _extrudedFrom = null;
      _grabbedBone = bone;
    }
    notifyListeners();
    return null;
  }

  /// One step of the grab: the joint follows render pixel ([x], [y]).
  void grabTo(double x, double y) {
    final bone = _grabbedBone;
    if (bone == null) return;
    if (rust.moveJointToCursor(
      bone: bone,
      x: x,
      y: y,
      saveUndo: false,
      mirror: _xMirror,
    )) {
      notifyListeners();
    }
  }

  /// A click: the joint stays where it is.
  void confirmGrab() {
    if (_grabbedBone == null) return;
    _grabbedBone = null;
    _extrudedFrom = null;
    markHintSeen('edit_grab');
    notifyListeners();
  }

  /// Esc or a right click: everything since the grab began is taken back, and
  /// a joint grown by E is gone.
  void cancelGrab() {
    if (_grabbedBone == null) return;
    rust.grabCancel();
    _grabbedBone = null;
    _selectedBone = _extrudedFrom ?? _selectedBone;
    markHintSeen('edit_grab');
    _extrudedFrom = null;
    notifyListeners();
  }

  bool _xMirror = false;

  /// Edit Mode's X-Axis Mirror (Blender's option of the same name): while on,
  /// moving a joint also moves the joint with the flipped name (`arm.L` and
  /// `arm.R`) to the other side. Working state: it is not saved in the project.
  bool get xMirror => _xMirror;

  void toggleXMirror() {
    _xMirror = !_xMirror;
    notifyListeners();
  }

  /// Edit Mode: drags joint [bone] onto render pixel ([x], [y]) and leaves it
  /// there. The bones below it stay where they are.
  void moveJoint(BigInt bone, double x, double y) {
    if (rust.moveJointToCursor(
      bone: bone,
      x: x,
      y: y,
      saveUndo: true,
      mirror: _xMirror,
    )) {
      notifyListeners();
    }
  }

    /// Edit Mode: makes a mirrored copy of the selected joint and every joint
  /// below it, on the other side of the centre of the object (left becomes
  /// right), and selects the copy of the selected joint. Returns a message.
  String symmetrizeSelected() {
    if (mode != 'edit') return 'Mirror works in Edit Mode';
    final bone = _selectedBone;
    if (bone == null) return 'Click a joint dot first, then mirror it';
    if (_grabbedBone != null) return 'Finish the move first: click, or press Esc';
    final copy = rust.symmetrizeBone(bone: bone);
    if (copy == null) {
      return 'Could not mirror that joint (is the object locked?)';
    }
    _selectedBone = copy;
    notifyListeners();
    return 'Mirrored bone $bone as bone $copy';
  }

  /// Edit Mode: deletes the selected bone. The bones below it join its parent.
  /// Nothing is selected afterwards, as in Blender. Returns a message.
  String deleteSelectedBone() {
    final bone = _selectedBone;
    if (bone == null) return 'Select a joint first: click its dot';
    if (_grabbedBone != null) return 'Finish the move first: click, or press Esc';
    if (!rust.deleteBone(bone: bone)) return 'Could not delete that bone';
    _selectedBone = null;
    notifyListeners();
    return 'Deleted bone $bone';
  }


  /// The name of bone [bone] of the active object, or null for an unknown bone.
  String? boneName(BigInt bone) => rust.boneName(bone: bone);

  /// Renames bone [bone] of the active object as one undo step. A name another
  /// bone has gets a number, and an empty name becomes the default name, as in
  /// Blender. Returns a message when the bone got a different name than the one
  /// typed, or when it could not be renamed.
  String? renameBone(BigInt bone, String name) {
    final given = rust.renameBone(bone: bone, name: name);
    if (given == null) {
      return 'Could not rename that bone (is the object locked?)';
    }
    notifyListeners();
    if (name.isNotEmpty && given != name) return 'The bone is called $given';
    return null;
  }

  void resetPose() {
    rust.resetPose();
    rust.refreshModels();
    notifyListeners();
  }

  void resetSkeleton() {
    rust.resetSkeleton();
    _selectedBone = null;
    _tool = StudioTool.placeBones;
    notifyListeners();
  }
}