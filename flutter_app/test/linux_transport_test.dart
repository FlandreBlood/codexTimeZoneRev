import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:codex_timezone/services/network_transport.dart';

void main() {
  test(
    'Linux proxy environment preserves routing without leaking credentials',
    () {
      final uri = Uri.parse('https://example.com/');
      expect(linuxProxyForUrl(uri, environment: {}), 'DIRECT');
      expect(
        linuxProxyForUrl(
          uri,
          environment: {'HTTPS_PROXY': 'http://127.0.0.1:10808'},
        ),
        'PROXY 127.0.0.1:10808',
      );
      expect(
        linuxProxyForUrl(
          uri,
          environment: {'ALL_PROXY': 'http://127.0.0.1:10808'},
        ),
        'PROXY 127.0.0.1:10808',
      );
      expect(
        linuxProxyForUrl(
          uri,
          environment: {
            'NO_PROXY': '.example.com',
            'ALL_PROXY': 'socks5://localhost:1',
          },
        ),
        'DIRECT',
      );
      expect(
        () => linuxProxyForUrl(
          Uri.parse('https://badexample.com'),
          environment: {
            'NO_PROXY': 'example.com',
            'ALL_PROXY': 'socks5://localhost:1',
          },
        ),
        throwsStateError,
      );
      for (final proxy in [
        'socks5://localhost:1080',
        'http://secret:password@localhost:80',
        'http://localhost:80/path',
      ]) {
        expect(
          () => linuxProxyForUrl(uri, environment: {'HTTPS_PROXY': proxy}),
          throwsStateError,
        );
      }
    },
  );
  test(
    'Linux HTTPS-only transport rejects plaintext before opening a socket',
    () async {
      final client = ProxyHttpTransport(
        (_) async => 'DIRECT',
        requireHttps: true,
      );
      try {
        await expectLater(
          client.get(Uri.parse('http://127.0.0.1:9/')),
          throwsFormatException,
        );
      } finally {
        client.close();
      }
    },
  );
  test('Redirect bodies also obey the streaming size limit', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    var requests = 0;
    final client = ProxyHttpTransport((_) async => 'DIRECT');
    server.listen((request) async {
      requests++;
      request.response.statusCode = HttpStatus.found;
      request.response.headers.set(HttpHeaders.locationHeader, '/next');
      request.response.add(List.filled(1024 * 1024 + 1, 65));
      await request.response.close();
    });
    try {
      await expectLater(
        client.get(Uri.parse('http://127.0.0.1:${server.port}')),
        throwsA(isA<HttpException>()),
      );
      expect(requests, 1);
    } finally {
      client.close();
      await server.close(force: true);
    }
  });
  test(
    'Linux transport rejects responses over the streaming size limit',
    () async {
      // Generic transport is used here solely to test the shared local stream.
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      final client = ProxyHttpTransport((_) async => 'DIRECT');
      server.listen((request) async {
        request.response.add(List.filled(1024 * 1024 + 1, 65));
        await request.response.close();
      });
      try {
        await expectLater(
          client.get(Uri.parse('http://127.0.0.1:${server.port}')),
          throwsA(isA<HttpException>()),
        );
      } finally {
        client.close();
        await server.close(force: true);
      }
    },
  );
}
