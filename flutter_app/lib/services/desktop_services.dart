import 'package:file_selector/file_selector.dart';
import 'package:flutter/services.dart';

abstract interface class DesktopServices {
  Future<String?> chooseClient(String platform);
  Future<void> copyVerified(String text);
}

class SystemDesktopServices implements DesktopServices {
  const SystemDesktopServices();
  @override
  Future<String?> chooseClient(String platform) async {
    final file = await openFile(
      acceptedTypeGroups: [
        platform == 'macos'
            ? const XTypeGroup(
                label: 'Codex 桌面客户端',
                uniformTypeIdentifiers: ['com.apple.application-bundle'],
              )
            : platform == 'linux'
            ? const XTypeGroup(label: 'Codex 桌面可执行程序')
            : const XTypeGroup(label: 'Codex 桌面客户端', extensions: ['exe']),
      ],
    );
    return file?.path;
  }

  @override
  Future<void> copyVerified(String text) async {
    await Clipboard.setData(ClipboardData(text: text));
    if ((await Clipboard.getData(Clipboard.kTextPlain))?.text != text) {
      throw StateError('系统剪贴板内容校验失败');
    }
  }
}
