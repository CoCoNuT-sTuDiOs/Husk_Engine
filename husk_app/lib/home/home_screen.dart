import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:husk_app/src/rust/api/simple.dart' as rust;
import 'package:husk_app/ui/husk_theme.dart';
import 'package:husk_app/ui/messages.dart';

/// The most width the start card takes.
const double _splashWidth = 920;

/// The height of the picture at the top of the start card.
const double _heroHeight = 250;

/// The page Husk opens on, laid out like Blender's start screen: a picture
/// across the top of a centred card, then a list of ways to start and a list of
/// places to get help. The projects Husk keeps for you sit in the left list,
/// one row per slot.
class HomeScreen extends StatefulWidget {
  const HomeScreen({
    super.key,
    required this.onNewProject,
    required this.onOpenProject,
    required this.onOpenFile,
    required this.onShowShortcuts,
    required this.onShowAbout,
    this.onBackToScene,
  });

  /// Starts a new project that will be saved at the given path in Husk's folder.
  final void Function(String path) onNewProject;

  /// Opens the project at the given path.
  final void Function(String path) onOpenProject;

  /// Asks for a project file anywhere on the computer and opens it.
  final VoidCallback onOpenFile;

  final VoidCallback onShowShortcuts;
  final VoidCallback onShowAbout;

  /// Set when Home was opened from a scene: goes back to that scene.
  final VoidCallback? onBackToScene;

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  List<rust.ProjectCard> _projects = const [];
  final Map<String, ui.Image> _thumbnails = {};
  int _limit = 2;

  @override
  void initState() {
    super.initState();
    _reload();
  }

  @override
  void dispose() {
    for (final image in _thumbnails.values) {
      image.dispose();
    }
    super.dispose();
  }

  void _reload() {
    final projects = rust.homeProjects();
    final limit = rust.homeProjectLimit().toInt();
    setState(() {
      _projects = projects;
      _limit = limit;
    });
    for (final project in projects) {
      if (!_thumbnails.containsKey(project.path)) _decodeThumbnail(project);
    }
  }

  void _decodeThumbnail(rust.ProjectCard project) {
    if (project.thumbRgba.isEmpty || project.thumbWidth == 0) return;
    ui.decodeImageFromPixels(
      project.thumbRgba,
      project.thumbWidth,
      project.thumbHeight,
      ui.PixelFormat.rgba8888,
      (image) {
        if (!mounted) {
          image.dispose();
          return;
        }
        setState(() => _thumbnails[project.path] = image);
      },
    );
  }

  void _newProject() {
    final path = rust.homeNewProjectPath();
    if (path == null) {
      showMessage(
        context,
        'Both project slots are used. Delete a project to make room.',
      );
      return;
    }
    widget.onNewProject(path);
  }

  Future<void> _delete(rust.ProjectCard project) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text('Delete "${project.name}"?'),
        content: const Text(
          'This deletes the project and its picture from this computer. '
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
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    final (deleted, message) = rust.homeDeleteProject(path: project.path);
    if (!mounted) return;
    if (deleted) {
      final image = _thumbnails.remove(project.path);
      WidgetsBinding.instance.addPostFrameCallback((_) => image?.dispose());
      _reload();
    }
    showMessage(context, message);
  }

  @override
  Widget build(BuildContext context) {
    final full = _projects.length >= _limit;
    return Scaffold(
      backgroundColor: HuskColors.app,
      body: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(32),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: _splashWidth),
            child: Material(
              color: HuskColors.panel,
              elevation: 12,
              shadowColor: Colors.black,
              clipBehavior: Clip.antiAlias,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(14),
                side: const BorderSide(color: HuskColors.border),
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const _HeroBanner(),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(32, 26, 32, 30),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(flex: 6, child: _leftColumn(full)),
                        const SizedBox(width: 44),
                        Expanded(flex: 4, child: _rightColumn()),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _leftColumn(bool full) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _SectionLabel('New'),
        _NewProjectButton(full: full, onTap: _newProject),
        const SizedBox(height: 4),
        _LinkRow(
          icon: Icons.folder_open_outlined,
          label: 'Open a file...',
          onTap: widget.onOpenFile,
        ),
        const SizedBox(height: 22),
        Row(
          children: [
            const _SectionLabel('Your projects'),
            const Spacer(),
            _SlotMeter(used: _projects.length, limit: _limit),
          ],
        ),
        for (var i = 0; i < _limit; i++) ...[
          if (i < _projects.length)
            _ProjectRow(
              project: _projects[i],
              image: _thumbnails[_projects[i].path],
              onOpen: () => widget.onOpenProject(_projects[i].path),
              onDelete: () => _delete(_projects[i]),
            )
          else
            _EmptyRow(onTap: _newProject),
          const SizedBox(height: 8),
        ],
      ],
    );
  }

  Widget _rightColumn() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const _SectionLabel('Getting started'),
        if (widget.onBackToScene != null)
          _LinkRow(
            icon: Icons.arrow_back,
            label: 'Back to your scene',
            onTap: widget.onBackToScene!,
          ),
        _LinkRow(
          icon: Icons.keyboard_outlined,
          label: 'Keyboard shortcuts',
          onTap: widget.onShowShortcuts,
        ),
        _LinkRow(
          icon: Icons.info_outline,
          label: 'About Husk',
          onTap: widget.onShowAbout,
        ),
      ],
    );
  }
}

/// "just now", "5 min ago", "3 h ago", "2 d ago".
String _agoText(int seconds) {
  if (seconds < 60) return 'just now';
  final minutes = seconds ~/ 60;
  if (minutes < 60) return '$minutes min ago';
  final hours = minutes ~/ 60;
  if (hours < 24) return '$hours h ago';
  return '${hours ~/ 24} d ago';
}

/// "412 B", "279 KB", "3.4 MB".
String _sizeText(int bytes) {
  if (bytes < 1024) return '$bytes B';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).round()} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}

/// The picture across the top of the card, with the Husk logo on its left and
/// the version on its right. It shows `assets/images/home_hero.jpg` (or
/// `home_hero.png`), and a plain placeholder until that picture exists.
class _HeroBanner extends StatelessWidget {
  const _HeroBanner();

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: _heroHeight,
      child: Stack(
        fit: StackFit.expand,
        children: [
          Image.asset(
            'assets/images/home_hero.jpg',
            fit: BoxFit.cover,
            errorBuilder: (context, error, stackTrace) => Image.asset(
              'assets/images/home_hero.png',
              fit: BoxFit.cover,
              errorBuilder: (context, error, stackTrace) =>
                  const _HeroPlaceholder(),
            ),
          ),
          // Darkens the top so the logo reads on any picture.
          const DecoratedBox(
            decoration: BoxDecoration(
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.center,
                colors: [Color(0xB3000000), Color(0x00000000)],
              ),
            ),
          ),
          Positioned(
            left: 24,
            top: 20,
            child: Row(
              children: [
                Image.asset('assets/images/husk_logo.png', height: 34),
                const SizedBox(width: 10),
                const Text(
                  'HUSK',
                  style: TextStyle(
                    color: HuskColors.orange,
                    fontWeight: FontWeight.w700,
                    letterSpacing: 2.5,
                    fontSize: 20,
                  ),
                ),
              ],
            ),
          ),
          Positioned(
            right: 20,
            top: 18,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
              decoration: BoxDecoration(
                color: const Color(0x73000000),
                borderRadius: BorderRadius.circular(12),
              ),
              child: const Text(
                'v0.1',
                style: TextStyle(
                  fontFamily: HuskText.mono,
                  fontSize: 12,
                  color: HuskColors.textPrimary,
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _HeroPlaceholder extends StatelessWidget {
  const _HeroPlaceholder();

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        gradient: LinearGradient(
          colors: [Color(0xFF4A2610), Color(0xFF1E1E1E)],
          begin: Alignment.topRight,
          end: Alignment.bottomLeft,
        ),
      ),
      alignment: Alignment.center,
      child: const Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            'Your hero picture goes here',
            style: TextStyle(fontSize: 16, color: HuskColors.textSecondary),
          ),
          SizedBox(height: 4),
          Text('assets/images/home_hero.jpg', style: HuskText.readout),
        ],
      ),
    );
  }
}

class _SectionLabel extends StatelessWidget {
  const _SectionLabel(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Text(
        text,
        style: const TextStyle(
          color: HuskColors.textSecondary,
          fontSize: 12,
          fontWeight: FontWeight.w500,
          letterSpacing: 0.6,
        ),
      ),
    );
  }
}

class _NewProjectButton extends StatelessWidget {
  const _NewProjectButton({required this.full, required this.onTap});

  final bool full;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final ink = full ? HuskColors.textSecondary : HuskColors.canvas;
    return Material(
      color: full ? HuskColors.elevated : HuskColors.orange,
      borderRadius: BorderRadius.circular(8),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(8),
        hoverColor: Colors.white.withValues(alpha: 0.10),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 13),
          child: Row(
            children: [
              Icon(Icons.add, size: 20, color: ink),
              const SizedBox(width: 10),
              Text(
                'New project',
                style: TextStyle(
                  fontSize: 15,
                  fontWeight: FontWeight.w700,
                  color: ink,
                ),
              ),
              const Spacer(),
              Text(
                full ? 'both slots used' : 'empty scene',
                style: TextStyle(fontSize: 12, color: ink.withValues(alpha: 0.75)),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _LinkRow extends StatelessWidget {
  const _LinkRow({
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(6),
        hoverColor: HuskColors.elevated,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
          child: Row(
            children: [
              Icon(icon, size: 18, color: HuskColors.textSecondary),
              const SizedBox(width: 12),
              Text(label, style: const TextStyle(fontSize: 14)),
            ],
          ),
        ),
      ),
    );
  }
}

/// "1 of 2 used", with a small pill for each slot.
class _SlotMeter extends StatelessWidget {
  const _SlotMeter({required this.used, required this.limit});

  final int used;
  final int limit;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Row(
        children: [
          for (var i = 0; i < limit; i++)
            Container(
              width: 22,
              height: 6,
              margin: const EdgeInsets.only(right: 4),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(3),
                color: i < used ? HuskColors.orange : HuskColors.border,
              ),
            ),
          const SizedBox(width: 8),
          Text('$used of $limit used', style: HuskText.readout),
        ],
      ),
    );
  }
}

/// One project in Husk's folder: its picture, name, when it changed and how big
/// it is. A tap opens it, and the menu can delete it.
class _ProjectRow extends StatelessWidget {
  const _ProjectRow({
    required this.project,
    required this.image,
    required this.onOpen,
    required this.onDelete,
  });

  final rust.ProjectCard project;
  final ui.Image? image;
  final VoidCallback onOpen;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final initial =
        project.name.isEmpty ? '?' : project.name.substring(0, 1).toUpperCase();
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: onOpen,
        borderRadius: BorderRadius.circular(8),
        hoverColor: HuskColors.elevated,
        child: Padding(
          padding: const EdgeInsets.all(8),
          child: Row(
            children: [
              ClipRRect(
                borderRadius: BorderRadius.circular(6),
                child: SizedBox(
                  width: 96,
                  height: 54,
                  child: image != null
                      ? RawImage(image: image, fit: BoxFit.cover)
                      : Container(
                          color: HuskColors.elevated,
                          alignment: Alignment.center,
                          child: Text(
                            initial,
                            style: TextStyle(
                              fontSize: 24,
                              fontWeight: FontWeight.w700,
                              color: HuskColors.orange.withValues(alpha: 0.7),
                            ),
                          ),
                        ),
                ),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      project.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 15,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      'Changed ${_agoText(project.secondsAgo.toInt())}'
                      '  ·  ${_sizeText(project.bytes.toInt())}',
                      style: HuskText.readout,
                    ),
                  ],
                ),
              ),
              PopupMenuButton<String>(
                tooltip: 'More',
                icon: const Icon(
                  Icons.more_horiz,
                  size: 18,
                  color: HuskColors.textSecondary,
                ),
                onSelected: (choice) {
                  if (choice == 'open') {
                    onOpen();
                  } else {
                    onDelete();
                  }
                },
                itemBuilder: (_) => const [
                  PopupMenuItem(value: 'open', child: Text('Open')),
                  PopupMenuItem(
                    value: 'delete',
                    child: Text(
                      'Delete',
                      style: TextStyle(color: HuskColors.error),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A free project slot: a plain outlined row that starts a new project.
class _EmptyRow extends StatelessWidget {
  const _EmptyRow({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      type: MaterialType.transparency,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(8),
        hoverColor: HuskColors.elevated,
        child: Container(
          height: 70,
          alignment: Alignment.center,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: HuskColors.border),
          ),
          child: const Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.add, size: 18, color: HuskColors.orange),
              SizedBox(width: 8),
              Text(
                'Empty slot',
                style: TextStyle(fontSize: 14, color: HuskColors.textSecondary),
              ),
            ],
          ),
        ),
      ),
    );
  }
}