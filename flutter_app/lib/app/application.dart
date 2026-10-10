import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:forui/forui.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../services/backend.dart';
import '../services/desktop_services.dart';
import '../services/network_info.dart';
import '../services/window_material.dart';
import '../state/launcher_controller.dart';
import 'launcher_page.dart';
import 'theme.dart';

class LauncherApplication extends StatefulWidget {
  const LauncherApplication({
    super.key,
    this.preview = false,
    this.backend,
    this.initialAppearance = 'system',
    this.desktop = const SystemDesktopServices(),
    this.networkFactory,
    this.acrylicEnabled = false,
  });
  final bool preview;
  final LauncherBackend? backend;
  final String initialAppearance;
  final DesktopServices desktop;
  final NetworkController Function()? networkFactory;
  final bool acrylicEnabled;
  @override
  State<LauncherApplication> createState() => _LauncherApplicationState();
}

class _LauncherApplicationState extends State<LauncherApplication> {
  late final launcher = LauncherController(
    widget.backend ??
        (widget.preview ? const PreviewBackend() : const NativeBackend()),
    preview: widget.preview,
  );
  late final network =
      widget.networkFactory?.call() ??
      NetworkController(
        preview: widget.preview,
        autoRefresh: !Platform.isLinux,
      );
  late final appearance = ValueNotifier(widget.initialAppearance);
  late bool acrylicEnabled = widget.acrylicEnabled;
  bool? materialDark;

  void _syncMaterial(bool dark) {
    if (!acrylicEnabled || materialDark == dark) return;
    materialDark = dark;
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      try {
        await WindowMaterial.apply(dark);
      } catch (_) {
        if (mounted) setState(() => acrylicEnabled = false);
      }
    });
  }

  @override
  void initState() {
    super.initState();
    launcher.initialize();
    network.initialize();
    if (!widget.preview) _loadAppearance();
  }

  Future<void> _loadAppearance() async {
    try {
      final saved = (await SharedPreferences.getInstance()).getString(
        'flutter-appearance',
      );
      if (mounted && ['system', 'light', 'dark'].contains(saved)) {
        setState(() => appearance.value = saved!);
      }
    } catch (_) {
      /* System appearance is a safe fallback. */
    }
  }

  Future<void> _setAppearance(String value) async {
    setState(() => appearance.value = value);
    if (!widget.preview) {
      try {
        await (await SharedPreferences.getInstance()).setString(
          'flutter-appearance',
          value,
        );
      } catch (e) {
        launcher.report('主题已切换，但无法保存偏好。', detail: e.toString());
      }
    }
  }

  @override
  Widget build(BuildContext context) => WidgetsApp(
    title: 'Codex 时区启动器',
    color: const Color(0xff18181b),
    debugShowCheckedModeBanner: false,
    locale: const Locale('zh', 'CN'),
    supportedLocales: FLocalizations.supportedLocales,
    localizationsDelegates: FLocalizations.localizationsDelegates,
    builder: (context, child) {
      final dark =
          appearance.value == 'dark' ||
          (appearance.value == 'system' &&
              MediaQuery.platformBrightnessOf(context) == Brightness.dark);
      _syncMaterial(dark);
      final theme = acrylicEnabled
          ? (dark ? darkAcrylicTheme : lightAcrylicTheme)
          : (dark ? darkLauncherTheme : lightLauncherTheme);
      return FTheme(
        data: theme,
        child: FToaster(
          child: FTooltipGroup(
            child: DefaultTextStyle(
              style: theme.typography.body.md.copyWith(
                color: theme.colors.foreground,
              ),
              child: child!,
            ),
          ),
        ),
      );
    },
    onGenerateRoute: (settings) => PageRouteBuilder<void>(
      settings: settings,
      pageBuilder: (context, animation, secondary) => ListenableBuilder(
        listenable: Listenable.merge([launcher, network, appearance]),
        builder: (context, _) => LauncherPage(
          desktop: widget.desktop,
          launcher: launcher,
          network: network,
          appearance: appearance.value,
          onAppearance: _setAppearance,
        ),
      ),
    ),
  );

  @override
  void dispose() {
    launcher.dispose();
    network.dispose();
    appearance.dispose();
    super.dispose();
  }
}
