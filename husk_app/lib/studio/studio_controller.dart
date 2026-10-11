import 'package:flutter/foundation.dart';

/* 
Studio interface state: what  is selected, which tool is active, and 
so on. Only the Dart-side interface state lives here; 
the Rust corestays the source of truth for the scene itself. 
Fields are added slice by slice.
*/

class StudioController extends ChangeNotifier {
  BigInt? _selectedBone;

  BigInt? get selectedBone => _selectedBone;

  void selectBone(BigInt? bone) {
    if (_selectedBone == bone) return;
    _selectedBone = bone;
    notifyListeners();
  }
}