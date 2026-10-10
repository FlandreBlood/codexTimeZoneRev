import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:timezone/data/latest_all.dart' as timezone;
import 'package:codex_timezone/app/application.dart';
import 'package:codex_timezone/services/backend.dart';
import 'package:codex_timezone/services/network_info.dart';

class LinuxPreview implements LauncherBackend {
  @override
  Future<Map<String, dynamic>> call(
    String command, [
    Map<String, dynamic> payload = const {},
  ]) async => {
    ...await const PreviewBackend().call(command, payload),
    'platform': 'linux',
    'detected': '/usr/lib/chatgpt/ChatGPT',
  };
}

void main() {
  testWidgets('Linux provides launch controls without skin controls', (
    tester,
  ) async {
    timezone.initializeTimeZones();
    SharedPreferences.setMockInitialValues({});
    tester.view.physicalSize = const Size(1280, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
    await tester.pumpWidget(
      LauncherApplication(
        backend: LinuxPreview(),
        networkFactory: () => NetworkController(preview: true),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('保存并启动'), findsOneWidget);
    expect(find.text('Dream Skin 兼容启动'), findsNothing);
    expect(find.text('打开 Dream Skin'), findsNothing);
    expect(tester.takeException(), isNull);
    await tester.pumpWidget(const SizedBox());
  });
}
