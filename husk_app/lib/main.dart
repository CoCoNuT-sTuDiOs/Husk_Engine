import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:husk_app/src/rust/api/simple.dart';
import 'package:husk_app/src/rust/frb_generated.dart';

const MethodChannel _huskChannel = MethodChannel('husk/texture');

Future<void> main() async {
  await RustLib.init();
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return const MaterialApp(
      debugShowCheckedModeBanner: false,
      home: Scaffold(body: HuskTextureView()),
    );
  }
}

class HuskTextureView extends StatefulWidget {
  const HuskTextureView({super.key});

  @override
  State<HuskTextureView> createState() => _HuskTextureViewState();
}

class _HuskTextureViewState extends State<HuskTextureView> {
  int? _textureId;
  BigInt? _lastBoneIndex;

  @override
  void initState() {
    super.initState();
    _loadTextureId();
  }

  Future<void> _loadTextureId() async {
    final int id = await _huskChannel.invokeMethod('getTextureId');
    setState(() {
      _textureId = id;
    });
  }

  void _handleTap(TapDownDetails details) {
    final result = placeBone(
      x: details.localPosition.dx,
      y: details.localPosition.dy,
      parent: _lastBoneIndex,
    );

    if (result != null) {
      final (index, x, y, z) = result;
      setState(() {
        _lastBoneIndex = index;
      });
      debugPrint(
        'Placed bone $index at (${x.toStringAsFixed(3)}, '
        '${y.toStringAsFixed(3)}, ${z.toStringAsFixed(3)}) '
        '— total bones: ${boneCount()}',
      );
    } else {
      debugPrint('Bone placement: click missed the mesh');
    }

  }


  void _handleComputeWeights() {
    final success = computeWeights();
    debugPrint(
      success
          ? 'Weights computed for ${boneCount()} bones'
          : 'Weight computation failed (no bones placed yet?)',
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_textureId == null) {
      return const Center(child: CircularProgressIndicator());
    }
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: 256,
            height: 256,
            child: GestureDetector(
              onTapDown: _handleTap,
              child: Texture(textureId: _textureId!),
            ),
          ),
          const SizedBox(height: 16),
          ElevatedButton(
            onPressed: _handleComputeWeights,
            child: const Text('Compute Weights'),
          ),
        ],
      ),
    );
  }
}