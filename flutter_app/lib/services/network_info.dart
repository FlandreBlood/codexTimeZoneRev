import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'network_transport.dart';

class IpInfo {
  const IpInfo({
    required this.ip,
    required this.timezone,
    required this.source,
    this.location = '',
    this.organization = '',
  });
  final String ip, timezone, source, location, organization;
  Map<String, dynamic> toJson() => {
    'ip': ip,
    'timezone': timezone,
    'source': source,
    'location': location,
    'organization': organization,
  };
  factory IpInfo.fromJson(Map<String, dynamic> json) {
    final ip = json['ip'] as String;
    final zone = json['timezone'] as String;
    if (InternetAddress.tryParse(ip) == null || zone.isEmpty) {
      throw const FormatException('无效网络缓存');
    }
    return IpInfo(
      ip: ip,
      timezone: zone,
      source: json['source'] as String,
      location: json['location'] as String? ?? '',
      organization: json['organization'] as String? ?? '',
    );
  }
}

typedef Provider = ({String name, String url, String kind});
const domesticProviders = <Provider>[
  (
    name: '腾讯',
    url: 'https://r.inews.qq.com/api/ip2city?otype=jsonp',
    kind: 'json',
  ),
  (name: 'IPIP.net', url: 'https://myip.ipip.net/json', kind: 'json'),
  (name: '又拍云', url: 'https://pubstatic.b0.upaiyun.com/?_upnode', kind: 'text'),
  (
    name: 'Cloudflare CN',
    url: 'https://www.cloudflare-cn.com/cdn-cgi/trace',
    kind: 'trace',
  ),
];
const internationalProviders = <Provider>[
  (name: 'AWS', url: 'https://checkip.amazonaws.com/', kind: 'text'),
  (
    name: 'Cloudflare Workers',
    url: 'https://workers.dev/cdn-cgi/trace',
    kind: 'trace',
  ),
  (
    name: 'Cloudflare US IPv4',
    url: 'https://1.0.0.1/cdn-cgi/trace',
    kind: 'trace',
  ),
  (name: 'IPify IPv4', url: 'https://api4.ipify.org?format=json', kind: 'json'),
  (name: 'IP.SB', url: 'https://api.ip.sb/geoip', kind: 'json'),
];

String parseIp(String text, String kind) {
  String? ip;
  if (kind == 'json') {
    final data = jsonDecode(
      text
          .replaceFirst(RegExp(r'^\s*\('), '')
          .replaceFirst(RegExp(r'\)\s*$'), ''),
    ) as Map<String, dynamic>;
    ip = data['ip'] as String?;
  } else if (kind == 'trace') {
    for (final line in text.split('\n')) {
      if (line.startsWith('ip=')) ip = line.substring(3).trim();
    }
  } else {
    ip = RegExp(r'^(?:\(?\s*)?(?:ip\s*[=:]\s*)?([0-9a-fA-F:.]+)(?:\s*\)?)$')
        .firstMatch(text.trim())
        ?.group(1);
  }
  if (ip == null || InternetAddress.tryParse(ip) == null) {
    throw const FormatException('服务未返回有效 IP');
  }
  return ip;
}

/// Completes with the first success, rather than the first failure.
Future<T> firstSuccess<T>(List<Future<T>> requests) {
  final result = Completer<T>();
  if (requests.isEmpty) return Future.error(StateError('没有可用查询源'));
  var failures = 0;
  for (final request in requests) {
    request.then(
      (value) {
        if (!result.isCompleted) result.complete(value);
      },
      onError: (Object error, StackTrace stack) {
        if (++failures == requests.length && !result.isCompleted) {
          result.completeError(StateError('所有 IP 查询源均不可用'), stack);
        }
      },
    );
  }
  return result.future;
}

class NetworkController extends ChangeNotifier {
  NetworkController({
    this.preview = false,
    this.autoRefresh = true,
    TransportFactory? transportFactory,
    this.requestTimeout = const Duration(seconds: 9),
  }) : _transportFactory = transportFactory ?? desktopTransport;
  final bool preview;
  final bool autoRefresh;
  final TransportFactory _transportFactory;
  final Duration requestTimeout;
  IpInfo? domestic, international;
  bool busy = false;
  bool cached = false;
  final Map<String, String> errors = {};
  final Set<NetworkTransport> _clients = {};
  bool _disposed = false;
  static const _cacheKey = 'flutter-network-v1';
  void _emit() {
    if (!_disposed) notifyListeners();
  }

  Future<void> initialize() async {
    if (preview) {
      domestic = const IpInfo(
        ip: '192.0.2.10',
        timezone: 'Asia/Shanghai',
        source: '演示数据',
        location: '中国 · 北京',
        organization: '示例网络',
      );
      international = const IpInfo(
        ip: '203.0.113.42',
        timezone: 'America/New_York',
        source: '演示数据',
        location: '美国 · 纽约',
        organization: 'Example Network',
      );
      _emit();
      return;
    }
    try {
      final raw = (await SharedPreferences.getInstance()).getString(_cacheKey);
      if (raw != null) {
        final data = jsonDecode(raw) as Map<String, dynamic>;
        domestic = data['domestic'] == null
            ? null
            : IpInfo.fromJson(data['domestic'] as Map<String, dynamic>);
        international = data['international'] == null
            ? null
            : IpInfo.fromJson(data['international'] as Map<String, dynamic>);
        cached = domestic != null || international != null;
        _emit();
      }
    } catch (_) {
      /* Corrupt optional cache must never block startup. */
    }
    if (!_disposed && autoRefresh) await refresh();
  }

  Future<String> _get(NetworkTransport client, String url) {
    // Includes system proxy resolution, redirects, headers and response body.
    return client
        .get(Uri.parse(url))
        .timeout(
          requestTimeout,
          onTimeout: () {
            client.close();
            throw TimeoutException('网络查询超时', requestTimeout);
          },
        );
  }

  Future<IpInfo> _query(Provider provider, NetworkTransport client) async {
    final ip = parseIp(await _get(client, provider.url), provider.kind);
    final geo = jsonDecode(
      await _get(
        client,
        'https://ip125.com/api/geo/${Uri.encodeComponent(ip)}',
      ),
    ) as Map<String, dynamic>;
    if (geo['status'] != 'success' ||
        geo['timezone'] is! String ||
        (geo['timezone'] as String).isEmpty) {
      throw const FormatException('归属地信息不完整');
    }
    return IpInfo(
      ip: ip,
      timezone: geo['timezone'] as String,
      source: provider.name,
      location: [
        geo['country'],
        geo['regionName'],
        geo['city'],
      ].whereType<String>().where((s) => s.isNotEmpty).toSet().join(' · '),
      organization: (geo['org'] ?? geo['isp'] ?? '') as String,
    );
  }

  Future<IpInfo> _region(List<Provider> providers) async {
    final clients = providers.map((_) => _transportFactory()).toList();
    _clients.addAll(clients);
    try {
      return await firstSuccess([
        for (var i = 0; i < providers.length; i++)
          _query(providers[i], clients[i]),
      ]);
    } finally {
      for (final client in clients) {
        client.close();
        _clients.remove(client);
      }
    }
  }

  Future<void> refresh() async {
    if (busy || preview || _disposed) return;
    busy = true;
    errors.clear();
    _emit();
    try {
      await Future.wait([
        (() async {
          try {
            domestic = await _region(domesticProviders);
          } catch (e) {
            domestic = null;
            errors['domestic'] = e.toString();
          }
        })(),
        (() async {
          try {
            international = await _region(internationalProviders);
          } catch (e) {
            international = null;
            errors['international'] = e.toString();
          }
        })(),
      ]);
      cached = false;
      if (!_disposed) {
        try {
          await (await SharedPreferences.getInstance()).setString(
            _cacheKey,
            jsonEncode({
              'domestic': domestic?.toJson(),
              'international': international?.toJson(),
            }),
          );
        } catch (_) {
          errors['cache'] = '网络信息已更新，但无法保存缓存。';
        }
      }
    } finally {
      busy = false;
      _emit();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    for (final client in _clients) {
      client.close();
    }
    _clients.clear();
    super.dispose();
  }
}
