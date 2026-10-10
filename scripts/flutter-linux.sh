#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
action="${1:-build}"
case "$action" in doctor|test|build|run|preview) ;; *) printf 'Unknown action: %s\n' "$action" >&2; exit 2;; esac
if [[ -d "$project_root/environment/linux-tools/bin" ]]; then
  export PATH="$project_root/environment/linux-tools/bin:$PATH"
  export PKG_CONFIG_PATH="$project_root/environment/linux-tools/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
  export CMAKE_PREFIX_PATH="$project_root/environment/linux-tools${CMAKE_PREFIX_PATH:+:$CMAKE_PREFIX_PATH}"
  export CODEX_TZ_C_COMPILER="$project_root/environment/linux-tools/bin/x86_64-conda-linux-gnu-clang"
  export CODEX_TZ_CXX_COMPILER="$project_root/environment/linux-tools/bin/x86_64-conda-linux-gnu-clang++"
  export CODEX_TZ_RUNTIME_LIB="$project_root/environment/linux-tools/lib"
fi
if [[ -x "$project_root/environment/flutter/bin/flutter" ]]; then
  export PATH="$project_root/environment/flutter/bin:$PATH"
fi
export CI=true
export CARGO_TARGET_DIR="$project_root/environment/flutter-cargo-target"
cd "$project_root/flutter_app"
if [[ "$action" == doctor ]]; then flutter doctor -v; exit; fi
flutter pub get --enforce-lockfile
if [[ "$action" == test ]]; then
  cargo test --locked --manifest-path "$project_root/native/launcher_core/Cargo.toml"
  flutter analyze
  flutter test --concurrency=1
  exit
fi
cargo build --release --locked --manifest-path "$project_root/native/launcher_core/Cargo.toml"
export CODEX_TZ_NATIVE_LIBRARY="$CARGO_TARGET_DIR/release/libcodex_timezone_core.so"
case "$action" in
  run) flutter run -d linux ;;
  preview) flutter run -d linux --dart-define=UI_PREVIEW=true ;;
  build)
    flutter build linux --release
    bundle="$project_root/flutter_app/build/linux/x64/release/bundle"
    artifacts="$project_root/environment/artifacts"
    mkdir -p "$artifacts"
    # The Flutter build bundle contains assets/libraries only. User settings live outside it.
    tar -czf "$artifacts/CodexTimeZone-Flutter-linux-x64.tar.gz" -C "$bundle" .
    printf 'Application: %s/codex_timezone\nArchive: %s/CodexTimeZone-Flutter-linux-x64.tar.gz\n' "$bundle" "$artifacts"
    ;;
esac
