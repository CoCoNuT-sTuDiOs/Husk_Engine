import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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
      home: Scaffold(
        body: HuskTextureView(),
      ),
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

  @override
  Widget build(BuildContext context) {
    if (_textureId == null) {
      return const Center(child: CircularProgressIndicator());
    }
    return Center(
      child: SizedBox(
        width: 256,
        height: 256,
        child: Texture(textureId: _textureId!),
      ),
    );
  }
}