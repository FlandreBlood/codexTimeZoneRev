#!/usr/bin/env python3
"""Isolated ABI/launch acceptance. Never starts or stops the real client."""
import ctypes
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time

root = Path(__file__).resolve().parents[1]
library = Path(os.environ.get('CODEX_TZ_NATIVE_LIBRARY', root / 'environment/flutter-cargo-target/release/libcodex_timezone_core.so'))
lib = ctypes.CDLL(str(library))
lib.launcher_abi_version.restype = ctypes.c_uint32
lib.launcher_call.argtypes = [ctypes.c_char_p]
lib.launcher_call.restype = ctypes.c_void_p
lib.launcher_free.argtypes = [ctypes.c_void_p]
assert lib.launcher_abi_version() == 1

def call(command, payload=None):
    pointer = lib.launcher_call(json.dumps({'version': 1, 'command': command, 'payload': payload or {}}).encode())
    try:
        return json.loads(ctypes.string_at(pointer).decode())
    finally:
        lib.launcher_free(pointer)

with tempfile.TemporaryDirectory(prefix='codex-tz-linux-') as temporary:
    folder = Path(temporary)
    folder.chmod(0o700)
    os.environ['XDG_CONFIG_HOME'] = str(folder / 'config')
    bootstrap = call('bootstrap')
    assert bootstrap['ok'] and bootstrap['data']['platform'] == 'linux', bootstrap
    settings = bootstrap['data']['settings']
    config = Path(bootstrap['data']['settingsPath'])
    assert config.parent.stat().st_mode & 0o777 == 0o700
    assert not call('repair_launch')['ok']
    assert not call('launch_dream_skin')['ok']
    assert not call('reapply_dream_skin')['ok']
    assert not call('validate', {'path': '/usr/lib/chatgpt/resources/codex'})['ok']
    print('ABI/bootstrap/private directory/unsupported actions: PASS')

    source = folder / 'fixture.rs'
    source.write_text('''fn main() {
        let tz = std::env::var("TZ").unwrap_or_default();
        let args = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
        let node = std::env::var("ELECTRON_RUN_AS_NODE").unwrap_or_default();
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let kde = std::env::var("KDE_SESSION_VERSION").unwrap_or_default();
        let bus = std::env::var("DBUS_SESSION_BUS_ADDRESS").unwrap_or_default();
        let home = std::env::var("HOME").unwrap_or_default();
        let codex_home = std::env::var("CODEX_HOME").unwrap_or_default();
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_default();
        std::fs::write("observed.txt", format!("{tz}\\n{args}\\n{node}\\n{desktop}\\n{kde}\\n{bus}\\n{home}\\n{codex_home}\\n{config_home}\\n")).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(30));
    }''')
    binary = folder / 'ChatGPT'
    subprocess.run(['rustc', str(source), '-o', str(binary)], check=True)
    binary.chmod(0o700)
    (folder / 'resources').mkdir(mode=0o700)
    (folder / 'resources/app.asar').write_text('isolated fixture')
    (folder / 'resources/app.asar').chmod(0o600)
    assert call('validate', {'path': str(binary)})['ok']
    settings.update(executable=str(binary), zoneId='America/New_York', dreamSkinCompatible=True)
    saved = call('save', {'settings': settings})
    assert saved['ok'] and not saved['data']['settings']['dreamSkinCompatible']
    assert config.stat().st_mode & 0o777 == 0o600
    assert call('bootstrap')['data']['settings']['zoneId'] == 'America/New_York'
    print('settings/private atomic save/skin disabled: PASS')

    original_tz = os.environ.get('TZ')
    os.environ['DISPLAY'] = os.environ.get('DISPLAY') or ':fixture'
    os.environ['ELECTRON_RUN_AS_NODE'] = '1'
    # Fixture metadata only: no connection to a real credential service.
    os.environ['XDG_CURRENT_DESKTOP'] = 'KDE'
    os.environ['KDE_SESSION_VERSION'] = '6'
    os.environ['DBUS_SESSION_BUS_ADDRESS'] = 'unix:path=/fixture/bus'
    os.environ['CODEX_HOME'] = str(folder / 'fixture-codex-home')
    result = call('launch', {'settings': settings})
    assert result['ok'], result
    pid = result['data']['pid']
    try:
        observation = (folder / 'observed.txt').read_text().splitlines()
        assert observation == ['America/New_York', '', '', 'KDE', '6',
                               'unix:path=/fixture/bus', os.environ['HOME'],
                               os.environ['CODEX_HOME'], os.environ['XDG_CONFIG_HOME']], observation
        assert os.environ.get('TZ') == original_tz
        assert call('check_launch', {'settings': settings})['data']['running']
        assert not call('launch', {'settings': settings})['ok']
        os.kill(pid, 0)
        print('fixture TZ/env/no debug flags/existing process preserved: PASS')
        print('desktop identity/bus/user profile environment preserved: PASS')
    finally:
        # Only this test's returned fixture PID is terminated.
        os.kill(pid, signal.SIGTERM)
        for _ in range(100):
            if not Path(f'/proc/{pid}').exists(): break
            time.sleep(0.01)
    print('real client was not launched or terminated')
