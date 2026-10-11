import 'package:flutter/foundation.dart';

/// Studio's interface state: what is selected, which tool is active, and
/// so on. Only the Dart-side interface state lives here; the Rust core
/// stays the source of truth for the scene itself. Fields are added slice
/// by slice.
class StudioController extends ChangeNotifier {
  BigInt? _selectedBone;
  bool _viewportMaximized = false;

  BigInt? get selectedBone => _selectedBone;

  /// True while the viewport fills the window and the other panes are hidden.
  bool get viewportMaximized => _viewportMaximized;

  void selectBone(BigInt? bone) {
    if (_selectedBone == bone) return;
    _selectedBone = bone;
    notifyListeners();
  }

  void toggleViewportMaximized() {
    _viewportMaximized = !_viewportMaximized;
    notifyListeners();
  }
}