import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:codex_timezone/services/network_info.dart';
import 'package:codex_timezone/services/network_transport.dart';

class FakeTransport implements NetworkTransport {
  FakeTransport(this.respond);
  final Future<String> Function(Uri uri) respond;
  bool closed = false;
  @override
  Future<String> get(Uri uri) => respond(uri);
  @override
  void close() {
    closed = true;
  }
}

void main() {
  setUp(() => SharedPreferences.setMockInitialValues({}));
  test(
    'manual-only startup opens no network transport but refresh still works',
    () async {
      var opened = 0;
      final controller = NetworkController(
        autoRefresh: false,
        transportFactory: () {
          opened++;
          return FakeTransport((_) async => throw StateError('offline'));
        },
      );
      addTearDown(controller.dispose);
      await controller.initialize();
      expect(opened, 0);
      await controller.refresh();
      expect(opened, 9);
    },
  );
  test('one region succeeds despite failure of all other region providers', () async {
    final transports = <FakeTransport>[];
    final controller = NetworkController(
      transportFactory: () {
        final transport = FakeTransport((uri) async {
          if (uri.host == 'r.inews.qq.com') return '{"ip":"192.0.2.1"}';
          if (uri.host == 'ip125.com') {
            return '{"status":"success","timezone":"Asia/Shanghai","country":"中国"}';
          }
          throw StateError('offline');
        });
        transports.add(transport);
        return transport;
      },
    );
    addTearDown(controller.dispose);
    await controller.refresh();
    expect(controller.domestic?.ip, '192.0.2.1');
    expect(controller.international, isNull);
    expect(controller.errors.keys, contains('international'));
    expect(controller.busy, isFalse);
    expect(transports.every((transport) => transport.closed), isTrue);
    final cache = (await SharedPreferences.getInstance()).getString(
      'flutter-network-v1',
    );
    expect((jsonDecode(cache!) as Map)['domestic']['ip'], '192.0.2.1');
  });

  test('timeouts close every losing transport and permit retry', () async {
    final transports = <FakeTransport>[];
    final controller = NetworkController(
      requestTimeout: const Duration(milliseconds: 10),
      transportFactory: () {
        final transport = FakeTransport((_) => Completer<String>().future);
        transports.add(transport);
        return transport;
      },
    );
    addTearDown(controller.dispose);
    await controller.refresh();
    expect(controller.errors.keys, containsAll(['domestic', 'international']));
    expect(transports.every((transport) => transport.closed), isTrue);
    await controller.refresh();
    expect(transports, hasLength(18));
    expect(controller.busy, isFalse);
  });
}
