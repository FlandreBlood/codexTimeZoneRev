#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
executable="$project_root/flutter_app/build/linux/x64/release/bundle/codex_timezone"
if [[ ! -x "$executable" ]]; then
  printf '请先运行 bash scripts/flutter-linux.sh build\n' >&2
  exit 1
fi
# The rootless compiler environment supplies this device's GTK runtime as well.
# The native backend removes loader overrides before starting the installed client.
if [[ -d "$project_root/environment/linux-tools/lib" ]]; then
  export LD_LIBRARY_PATH="$project_root/environment/linux-tools/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi
unset CODEX_TZ_NATIVE_LIBRARY ELECTRON_RUN_AS_NODE
exec "$executable" "$@"
