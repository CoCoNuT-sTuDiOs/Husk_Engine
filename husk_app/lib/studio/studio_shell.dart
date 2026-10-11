import 'package:flutter/material.dart';
import 'package:husk_app/studio/studio_controller.dart';
import 'package:husk_app/studio/studio_theme.dart';
import 'package:husk_app/studio/timeline_controller.dart';
import 'package:husk_app/studio/timeline_pane.dart';
import 'package:husk_app/studio/viewport_pane.dart';
import 'package:phosphor_icons/phosphor_icons.dart';


class StudioShell extends StatefulWidget {
  const StudioShell({super.key});

  @override
  State<StudioShell> createState() => _StudioShellState();
}

class _StudioShellState extends State<StudioShell> {
  final StudioController _controller = StudioController();
  // Keeps the viewport's state (texture, size) when it moves between the
  // normal layout and the maximized one.
  final GlobalKey _viewportKey = GlobalKey();
  final TimelineController _timeline = TimelineController();

  @override
  void dispose() {
    _controller.dispose();
    _timeline.dispose();
    super.dispose();
  }
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Column(
        children: [
          const _TopBar(),
          Expanded(
            child: ListenableBuilder(
              listenable: _controller,
              builder: (context, _) {
                final viewport = ViewportPane(
                  key: _viewportKey,
                  controller: _controller,
                );
                if (_controller.viewportMaximized) {
                  return viewport;
                }
                return Row(
                  children: [
                    const _ToolColumn(),
                    Expanded(
                      child: Column(
                        children: [
                          Expanded(child: viewport),
                          TimelinePane(timeline: _timeline, studio: _controller),
                        ],
                      ),
                    ),
                    const _SidePanel(),
                  ],
                );
              },
            ),
          ),
        ],
      ),
    );
  }
}


class _TopBar extends StatelessWidget {
  const _TopBar();

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 44,
      padding: const EdgeInsets.symmetric(horizontal: 14),
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(bottom: BorderSide(color: HuskColors.border)),
      ),
      child: Row(
        children: [
          Image.asset('assets/images/husk_logo.png', height: 28),
          const SizedBox(width: 10),
          const Text(
            'HUSK',
            style: TextStyle(
              color: HuskColors.orange,
              fontWeight: FontWeight.w700,
              letterSpacing: 2,
              fontSize: 16,
            ),
          ),
          const SizedBox(width: 18),
          const Text(
            'Untitled project',
            style: TextStyle(color: HuskColors.textSecondary, fontSize: 13),
          ),
          const Spacer(),
          // Disabled until Section 7 (export) exists.
          FilledButton.icon(
            onPressed: null,
            icon: PhosphorIcon(PhosphorIconsRegular.export, size: 18),
            label: const Text('Export'),
          ),
        ],
      ),
    );
  }
}

class _ToolColumn extends StatelessWidget {
  const _ToolColumn();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 52,
      padding: const EdgeInsets.symmetric(vertical: 8),
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(right: BorderSide(color: HuskColors.border)),
      ),
      child: Column(
        children: [
          _ToolButton(
            icon: PhosphorIcon(PhosphorIconsRegular.cursor),
            tooltip: 'Select',
            selected: true,
          ),
          _ToolButton(
            icon: PhosphorIcon(PhosphorIconsRegular.bone),
            tooltip: 'Place bones',
          ),
          _ToolButton(
            icon: PhosphorIcon(PhosphorIconsRegular.arrowsOutCardinal),
            tooltip: 'Move',
          ),
        ],
      ),
    );
  }
}

class _ToolButton extends StatelessWidget {
  const _ToolButton({
    required this.icon,
    required this.tooltip,
    this.selected = false,
  });

  final Widget icon;
  final String tooltip;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: Container(
        width: 40,
        height: 40,
        margin: const EdgeInsets.symmetric(vertical: 2),
        decoration: BoxDecoration(
          color: selected
              ? HuskColors.blue.withValues(alpha: 0.2)
              : Colors.transparent,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(
            color: selected ? HuskColors.blue : Colors.transparent,
          ),
        ),
        child: IconTheme(
          data: IconThemeData(
            color: selected ? HuskColors.blue : HuskColors.textSecondary,
            size: 22,
          ),
          child: Center(child: icon),
        ),
      ),
    );
  }
}


class _SidePanel extends StatelessWidget {
  const _SidePanel();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 300,
      decoration: const BoxDecoration(
        color: HuskColors.panel,
        border: Border(left: BorderSide(color: HuskColors.border)),
      ),
      child: const Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _PanelHeader(title: 'Outliner'),
          Expanded(
            child: Center(
              child: Text('Objects arrive in Section 5c', style: HuskText.readout),
            ),
          ),
          _PanelHeader(title: 'Properties'),
          Expanded(
            child: Center(
              child: Text('Tools and settings arrive in UI-1d', style: HuskText.readout),
            ),
          ),
        ],
      ),
    );
  }
}

class _PanelHeader extends StatelessWidget {
  const _PanelHeader({required this.title, this.trailing});

  final String title;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 32,
      padding: const EdgeInsets.symmetric(horizontal: 12),
      decoration: const BoxDecoration(
        color: HuskColors.elevated,
        border: Border(bottom: BorderSide(color: HuskColors.border)),
      ),
      child: Row(
        children: [
          Text(
            title,
            style: const TextStyle(
              color: HuskColors.textPrimary,
              fontSize: 13,
              fontWeight: FontWeight.w500,
            ),
          ),
          const Spacer(),
          if (trailing != null) trailing!,
        ],
      ),
    );
  }
}