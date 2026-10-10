import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:path/path.dart' as p;

abstract interface class LauncherBackend {
  Future<Map<String, dynamic>> call(
    String command, [
    Map<String, dynamic> payload = const {},
  ]);
}

class NativeBackend implements LauncherBackend {
  const NativeBackend();

  @override
  Future<Map<String, dynamic>> call(
    String command, [
    Map<String, dynamic> payload = const {},
  ]) {
    final request = jsonEncode({
      'version': 1,
      'command': command,
      'payload': payload,
    });
    return Isolate.run(() => _invoke(request));
  }
}

Map<String, dynamic> _invoke(String request) {
  final executableDirectory = p.dirname(Platform.resolvedExecutable);
  final override = Platform.environment['CODEX_TZ_NATIVE_LIBRARY'];
  final path =
      override ??
      (Platform.isLinux
          ? p.join(executableDirectory, 'lib', 'libcodex_timezone_core.so')
          : Platform.isWindows
          ? p.join(executableDirectory, 'codex_timezone_core.dll')
          : p.normalize(
              p.join(
                executableDirectory,
                '../Frameworks/libcodex_timezone_core.dylib',
              ),
            ));
  if (!File(path).existsSync()) {
    throw StateError('缺少原生组件，请使用项目构建脚本重新构建。\n$path');
  }
  final library = DynamicLibrary.open(path);
  final version = library.lookupFunction<Uint32 Function(), int Function()>(
    'launcher_abi_version',
  );
  if (version() != 1) throw StateError('原生组件版本不匹配，请重新构建。');
  final call = library
      .lookupFunction<
        Pointer<Utf8> Function(Pointer<Utf8>),
        Pointer<Utf8> Function(Pointer<Utf8>)
      >('launcher_call');
  final free = library
      .lookupFunction<
        Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)
      >('launcher_free');
  final input = request.toNativeUtf8();
  Pointer<Utf8> output = nullptr;
  try {
    output = call(input);
    if (output == nullptr) throw StateError('原生组件未返回结果。');
    final response = jsonDecode(output.toDartString()) as Map<String, dynamic>;
    if (response['version'] != 1) throw StateError('原生响应版本不匹配。');
    if (response['ok'] != true) {
      throw StateError(response['error'] as String? ?? '系统操作失败。');
    }
    return response['data'] as Map<String, dynamic>;
  } finally {
    calloc.free(input);
    if (output != nullptr) free(output);
  }
}

class PreviewBackend implements LauncherBackend {
  const PreviewBackend();
  @override
  Future<Map<String, dynamic>> call(
    String command, [
    Map<String, dynamic> payload = const {},
  ]) async {
    if (command != 'bootstrap') throw StateError('演示模式不执行系统操作。');
    return {
      'settings': {
        'mode': 'zone',
        'zoneId': 'Asia/Shanghai',
        'offset': 8,
        'executable': '',
        'dreamSkinCompatible': false,
      },
      'zones': [
        {'id': 'Asia/Shanghai', 'label': '中国标准时间（上海 / 北京）'},
        {'id': 'America/New_York', 'label': '美国东部时间（纽约）'},
        {'id': 'Asia/Kolkata', 'label': '印度标准时间'},
        {'id': 'Asia/Kathmandu', 'label': '尼泊尔时间'},
      ],
      'detected': Platform.isLinux
          ? '/usr/lib/chatgpt/ChatGPT（演示）'
          : Platform.isMacOS
          ? '/Applications/Codex.app（演示）'
          : r'C:\Program Files\Codex\Codex.exe（演示）',
      'platform': Platform.isLinux
          ? 'linux'
          : Platform.isMacOS
          ? 'macos'
          : 'windows',
    };
  }
}
