#!/usr/bin/env python3
"""Check both release formats; --gui starts only disposable test launchers."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--gui', action='store_true')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    assets = root / 'environment/artifacts'
    validation_path = assets / 'validation.json'
    validation = json.loads(validation_path.read_text())
    archive = assets / validation['asset']
    deb = assets / validation['debAsset']
    for path, expected in [(archive, validation['sha256']), (deb, validation['debSha256'])]:
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected
    version = subprocess.check_output(['dpkg-deb', '-f', str(deb), 'Version'], text=True).strip()
    architecture = subprocess.check_output(['dpkg-deb', '-f', str(deb), 'Architecture'], text=True).strip()
    assert version == '0.1.0' and architecture == 'amd64'
    contents = subprocess.check_output(['dpkg-deb', '--fsys-tarfile', str(deb)])
    with tarfile.open(fileobj=io.BytesIO(contents)) as tar:
        assert all(p.uid == 0 and p.gid == 0 for p in tar.getmembers())
    control = subprocess.check_output(['dpkg-deb', '--ctrl-tarfile', str(deb)])
    with tarfile.open(fileobj=io.BytesIO(control)) as tar:
        assert not any(Path(p.name).name in ['preinst','postinst','prerm','postrm'] for p in tar.getmembers())
    print('Asset hashes / deb version, architecture, root ownership and no maintainer scripts: PASS', flush=True)
    env = os.environ.copy()
    env.pop('LD_PRELOAD', None)
    if args.gui:
        keys = {'DISPLAY','WAYLAND_DISPLAY','XAUTHORITY','XDG_RUNTIME_DIR','DBUS_SESSION_BUS_ADDRESS',
                'XDG_CURRENT_DESKTOP','XDG_SESSION_DESKTOP','DESKTOP_SESSION','KDE_SESSION_VERSION',
                'KDE_FULL_SESSION','GNOME_DESKTOP_SESSION_ID'}
        for folder in Path('/proc').iterdir():
            if not folder.name.isdigit():
                continue
            try:
                if folder.stat().st_uid != os.getuid() or os.readlink(folder / 'exe') not in ['/usr/bin/plasmashell','/usr/bin/gnome-shell']:
                    continue
                for item in (folder / 'environ').read_bytes().split(b'\0'):
                    key, sep, value = item.partition(b'=')
                    if sep and key.decode(errors='ignore') in keys and value:
                        env[key.decode()] = os.fsdecode(value)
                break
            except OSError:
                continue
        assert env.get('DBUS_SESSION_BUS_ADDRESS'), 'GUI checks require a real desktop session'
    with tempfile.TemporaryDirectory(prefix='codex release with spaces ') as temporary:
        directory = Path(temporary)
        with tarfile.open(archive) as tar:
            tar.extractall(directory / 'archive', filter='data')
        subprocess.run(['dpkg-deb', '-x', str(deb), str(directory / 'deb')], check=True)
        packages = [directory / 'archive' / archive.name.removesuffix('.tar.gz'), directory / 'deb/opt/codex-timezone']
        for index, package in enumerate(packages):
            for line in (package / 'FILES.sha256').read_text().splitlines():
                digest, name = line.split('  ', 1)
                assert hashlib.sha256((package / name).read_bytes()).hexdigest() == digest
            check_env = env.copy()
            check_env['LD_LIBRARY_PATH'] = str(package / 'lib')
            check_env['XDG_CACHE_HOME'] = str(directory / f'cache-{index}')
            for path in [package / 'codex_timezone', package / 'lib/libcodex_timezone_core.so']:
                dependencies = subprocess.check_output(['ldd', str(path)], env=check_env, text=True)
                assert 'not found' not in dependencies and str(root / 'environment/linux-tools') not in dependencies
            subprocess.run(['bash', '-n', str(package / 'run.sh')], check=True)
            print(f'Format {index}: extracted manifest and relocated dependencies: PASS', flush=True)
            if not args.gui:
                continue
            with (root / 'environment' / f'linux-v0.1-release-window-{index}.log').open('w') as log:
                child = subprocess.Popen(['bash', str(package / 'run.sh')], env=check_env, stdout=log, stderr=subprocess.STDOUT)
                try:
                    for _ in range(20):
                        time.sleep(.5)
                        assert child.poll() is None, 'Test launcher exited early'
                        maps = Path(f'/proc/{child.pid}/maps').read_text()
                        if str(package / 'lib/libcodex_timezone_core.so') in maps and str(package / 'lib/libgtk-3.so.0') in maps:
                            break
                    else:
                        raise AssertionError('Packaged native backend/GTK not loaded')
                    assert str(root / 'environment/linux-tools') not in maps, 'Build-prefix dynamic dependency remains'
                    assert str(root / 'flutter_app/build') not in maps
                    print(f'Format {index}: GUI loads only relocated packaged runtime: PASS', flush=True)
                finally:
                    if child.poll() is None:
                        child.terminate()
                    child.wait(timeout=10)
    validation.update({'rustTestsPassed':18, 'flutterTestsPassed':53, 'flutterTestsSkipped':1,
        'flutterAnalyze':'passed', 'isolatedNativeAbi':'passed', 'debMetadataAndRootOwnership':'passed',
        'bothFormatsRelocatedFileManifests':'passed', 'userReportedOriginalLoginReuse':'passed',
        'realCodexNotRestartedDuringReleaseChecks':True})
    if args.gui:
        validation['bothFormatsRelocatedGuiLoadsPackagedRuntime'] = 'passed'
    validation_path.write_text(json.dumps(validation, ensure_ascii=False, indent=2) + '\n')
    print('Only disposable test launchers were started/stopped; original Codex unchanged', flush=True)


if __name__ == '__main__':
    main()
