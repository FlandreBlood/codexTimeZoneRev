#!/usr/bin/env python3
"""Package the verified Linux bundle without build-machine runtime paths."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', default='v0.1')
    args = parser.parse_args()
    if not re.fullmatch(r'v[0-9]+(?:\.[0-9]+){1,2}', args.version):
        parser.error('version must be a numeric v-prefixed release tag')
    root = Path(__file__).resolve().parents[1]
    prefix = root / 'environment/linux-tools'
    bundle = root / 'flutter_app/build/linux/x64/release/bundle'
    tool = shutil.which('patchelf') or str(prefix / 'bin/patchelf')
    if not Path(tool).is_file() or not (bundle / 'codex_timezone').is_file():
        raise RuntimeError('Build Linux first and install patchelf')
    name = f'CodexTimeZone-Linux-x86_64-{args.version}'
    stage = root / 'environment/release' / args.version / name
    if stage.exists():
        shutil.rmtree(stage)
    stage.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(bundle, stage)
    # GTK's compiled module-cache location points to the conda build prefix.
    # Bundle its modules/query helper and generate the cache at launch instead.
    module_source = prefix / 'lib/gtk-3.0/3.0.0/immodules'
    if module_source.is_dir():
        shutil.copytree(module_source, stage / 'lib/gtk-immodules')
        (stage / 'libexec').mkdir()
        shutil.copyfile(prefix / 'bin/gtk-query-immodules-3.0', stage / 'libexec/gtk-query-immodules-3.0')
        (stage / 'libexec/gtk-query-immodules-3.0').chmod(0o755)
    metadata = [json.loads(p.read_text()) for p in (prefix / 'conda-meta').glob('*.json')]
    packaged = {}
    env = os.environ.copy()
    env.pop('LD_PRELOAD', None)
    env['LD_LIBRARY_PATH'] = str(stage / 'lib') + ':' + str(prefix / 'lib')

    def elfs():
        for path in stage.rglob('*'):
            if path.is_file():
                with path.open('rb') as stream:
                    if stream.read(4) == b'\x7fELF':
                        yield path

    def dependencies(path, environment):
        output = subprocess.check_output(['ldd', str(path)], env=environment, text=True)
        if 'not found' in output:
            raise RuntimeError(f'Missing dependency of {path.name}')
        return [(m[0], Path(m[1])) for m in re.findall(r'^\s*(\S+) => (/\S+) ', output, re.M)]

    # ldd traverses DT_NEEDED; scan plugins and the FFI library as well as runner.
    scanned = set()
    while True:
        pending = [p for p in elfs() if p not in scanned]
        if not pending:
            break
        for path in pending:
            scanned.add(path)
            for soname, source in dependencies(path, env):
                try:
                    relative = source.resolve().relative_to(prefix.resolve()).as_posix()
                except ValueError:
                    continue
                destination = stage / 'lib' / soname
                if not destination.exists():
                    shutil.copyfile(source, destination)
                    destination.chmod(0o755)
                for m in metadata:
                    if relative in m.get('files', []):
                        packaged[m['name']] = m

    for path in elfs():
        relative = os.path.relpath(stage / 'lib', path.parent)
        rpath = '$ORIGIN' if relative == '.' else '$ORIGIN/' + relative
        subprocess.run([tool, '--set-rpath', rpath, str(path)], check=True)
    clean_env = env.copy()
    clean_env['LD_LIBRARY_PATH'] = str(stage / 'lib')
    glibc_versions = set()
    for path in elfs():
        for _, dependency in dependencies(path, clean_env):
            if prefix.resolve() in dependency.resolve().parents:
                raise RuntimeError('Package still depends on local conda prefix')
        rpath = subprocess.check_output([tool, '--print-rpath', str(path)], text=True).strip()
        if str(root) in rpath or not rpath.startswith('$ORIGIN'):
            raise RuntimeError('Package contains a build-machine RPATH')
        versions = subprocess.check_output(['readelf', '--version-info', str(path)], text=True)
        glibc_versions.update(re.findall(r'\bGLIBC_([0-9]+\.[0-9]+)\b', versions))

    licenses = stage / 'licenses'
    licenses.mkdir()
    shutil.copyfile(root / 'LICENSE', licenses / 'project-LICENSE')
    flutter_license = root / 'environment/flutter/LICENSE'
    if flutter_license.exists():
        shutil.copyfile(flutter_license, licenses / 'Flutter-LICENSE')
    source_records = []
    for name_, m in sorted(packaged.items()):
        target = licenses / name_
        target.mkdir()
        cached = Path(m.get('link', {}).get('source', ''))
        cached_licenses = cached / 'info/licenses'
        if cached_licenses.is_dir():
            shutil.copytree(cached_licenses, target, dirs_exist_ok=True)
        # conda-forge splits FreeType's shared library and license into outputs.
        if name_ == 'libfreetype6' and not any(p.is_file() for p in target.rglob('*')):
            for sibling in metadata:
                if sibling['name'] in ['freetype', 'libfreetype'] and sibling['version'] == m['version']:
                    sibling_licenses = Path(sibling.get('link', {}).get('source', '')) / 'info/licenses'
                    if sibling_licenses.is_dir():
                        shutil.copytree(sibling_licenses, target, dirs_exist_ok=True)
        for filename in m.get('files', []):
            if filename.startswith('share/licenses/') and (prefix / filename).is_file():
                shutil.copyfile(prefix / filename, target / Path(filename).name)
        if 'GPL-3.0' in m.get('license', ''):
            common = Path('/usr/share/common-licenses/GPL-3')
            if common.exists():
                shutil.copyfile(common, target / 'GPL-3.txt')
        urls = []
        recipe = cached / 'info/recipe/meta.yaml'
        if recipe.exists():
            text = recipe.read_text()
            urls = sorted(set(re.findall(r'https?://[^\s\"\'<>]+', text)))
        source_records.append({'name':name_, 'version':m['version'], 'license':m.get('license'),
            'sources':urls, 'feedstock':f'https://github.com/conda-forge/{name_}-feedstock',
            'licenseFiles':[str(p.relative_to(stage)) for p in target.rglob('*') if p.is_file()]})
        if not source_records[-1]['licenseFiles']:
            raise RuntimeError(f'Missing runtime license text: {name_}')
    (stage / 'THIRD_PARTY.json').write_text(json.dumps(source_records, ensure_ascii=False, indent=2) + '\n')
    (stage / 'run.sh').write_text('''#!/usr/bin/env bash
set -euo pipefail
release_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$release_root/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
unset CODEX_TZ_NATIVE_LIBRARY ELECTRON_RUN_AS_NODE
export FONTCONFIG_FILE="${FONTCONFIG_FILE:-/etc/fonts/fonts.conf}"
export FONTCONFIG_PATH="${FONTCONFIG_PATH:-/etc/fonts}"
if [[ -x "$release_root/libexec/gtk-query-immodules-3.0" ]]; then
  runtime_cache="${XDG_CACHE_HOME:-$HOME/.cache}/com.fei-away.codextimezone"
  mkdir -p -m 700 "$runtime_cache"
  cache_id="$(printf '%s' "$release_root" | sha256sum | cut -c1-16)"
  cache_file="$(mktemp "$runtime_cache/immodules.XXXXXX")"
  "$release_root/libexec/gtk-query-immodules-3.0" "$release_root/lib/gtk-immodules/"*.so > "$cache_file"
  mv -f "$cache_file" "$runtime_cache/immodules-$cache_id.cache"
  export GTK_IM_MODULE_FILE="$runtime_cache/immodules-$cache_id.cache"
  export CODEX_TZ_RUNTIME_CACHE="$runtime_cache"
fi
exec "$release_root/codex_timezone" "$@"
''')
    (stage / 'run.sh').chmod(0o755)
    shutil.copyfile(root / 'docs/Linux-v0.1发布说明.md', stage / 'README.zh-CN.md')
    hashes = []
    for path in sorted(stage.rglob('*')):
        if path.is_file():
            hashes.append(hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + path.relative_to(stage).as_posix())
    (stage / 'FILES.sha256').write_text('\n'.join(hashes) + '\n')
    artifacts = root / 'environment/artifacts'
    artifacts.mkdir(exist_ok=True)
    archive = artifacts / f'{stage.name}.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        tar.add(stage, arcname=stage.name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    deb_root = stage.parent / 'deb-root'
    if deb_root.exists():
        shutil.rmtree(deb_root)
    application = deb_root / 'opt/codex-timezone'
    application.parent.mkdir(parents=True)
    shutil.copytree(stage, application)
    desktop = deb_root / 'usr/share/applications/codex-timezone.desktop'
    desktop.parent.mkdir(parents=True)
    desktop.write_text('[Desktop Entry]\nType=Application\nName=Codex 时区启动器 (Linux)\nComment=沿用原 Codex 登录并设置启动时区\nExec=/opt/codex-timezone/run.sh\nIcon=codex-timezone\nTerminal=false\nCategories=Utility;\n')
    icon = deb_root / 'usr/share/icons/hicolor/256x256/apps/codex-timezone.png'
    icon.parent.mkdir(parents=True)
    shutil.copyfile(root / 'flutter_app/assets/app_icon.png', icon)
    binary = deb_root / 'usr/bin/codex-timezone'
    binary.parent.mkdir(parents=True)
    binary.write_text('#!/usr/bin/env bash\nexec /opt/codex-timezone/run.sh "$@"\n')
    binary.chmod(0o755)
    control = deb_root / 'DEBIAN/control'
    control.parent.mkdir()
    installed_size = sum(p.stat().st_size for p in application.rglob('*') if p.is_file()) // 1024
    version = args.version[1:] + ('.0' if args.version.count('.') == 1 else '')
    control.write_text(f'Package: codex-timezone\nVersion: {version}\nArchitecture: amd64\nMaintainer: FlandreBlood <FlandreBlood@users.noreply.github.com>\nSection: utils\nPriority: optional\nInstalled-Size: {installed_size}\nDepends: libc6 (>= 2.39), libepoxy0, libfontconfig1, libegl1, libgl1, libgles2, tzdata\nHomepage: https://github.com/FlandreBlood/codexTimeZoneRev\nDescription: Codex timezone launcher for Linux\n Reuses the installed Codex desktop environment and user data.\n No skin integration or system timezone change.\n')
    deb = artifacts / f'CodexTimeZone-Linux-amd64-{args.version}.deb'
    subprocess.run(['dpkg-deb', '--root-owner-group', '--build', str(deb_root), str(deb)], check=True)
    deb_digest = hashlib.sha256(deb.read_bytes()).hexdigest()
    (artifacts / 'SHA256SUMS').write_text(f'{deb_digest}  {deb.name}\n{digest}  {archive.name}\n')
    validation = {'release':args.version, 'platform':'linux-x86_64',
        'minimumGlibc':max(glibc_versions, key=lambda v:tuple(map(int,v.split('.')))),
        'relativeRuntimePaths':True, 'dependencyResolutionWithoutBuildPrefix':True,
        'runtimePackages':len(packaged), 'asset':archive.name, 'sha256':digest,
        'debAsset':deb.name, 'debSha256':deb_digest,
        'scope':'本机 Ubuntu/KDE 验证；其他发行版未实测；不包含用户配置或账户数据'}
    (artifacts / 'validation.json').write_text(json.dumps(validation, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(validation, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
