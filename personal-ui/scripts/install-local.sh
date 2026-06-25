#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Install the GoXLR personal native UI into the current user's local desktop session.

Usage: personal-ui/scripts/install-local.sh [--no-build] [--debug]

Options:
  --no-build   Reuse an existing target binary instead of building first.
  --debug      Install target/debug/goxlr-personal-ui instead of target/release/goxlr-personal-ui.
  -h, --help   Show this help.

Installs:
  ~/.local/bin/goxlr-personal-ui
  ~/.local/share/applications/goxlr-personal-ui.desktop
USAGE
}

build=true
profile=release
cargo_profile_args=(--release)

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build)
      build=false
      ;;
    --debug)
      profile=debug
      cargo_profile_args=()
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
package_name="goxlr-personal-ui"
binary_source="$repo_root/target/$profile/$package_name"
bin_dir="${XDG_BIN_HOME:-$HOME/.local/bin}"
app_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
binary_target="$bin_dir/$package_name"
desktop_target="$app_dir/$package_name.desktop"

cd "$repo_root"

if [[ "$build" == true ]]; then
  if [[ "$profile" == "release" ]]; then
    cargo build -p goxlr-personal-ui --release --features system-tray
  else
    cargo build -p goxlr-personal-ui --features system-tray
  fi
fi

if [[ ! -x "$binary_source" ]]; then
  echo "Built binary not found or not executable: $binary_source" >&2
  exit 1
fi

mkdir -p "$bin_dir" "$app_dir"
install -m 0755 "$binary_source" "$binary_target"

cat > "$desktop_target" <<DESKTOP
[Desktop Entry]
Type=Application
Name=GoXLR Personal Control
Comment=Personal native Rust control panel for GoXLR Utility
Exec=$binary_target
Terminal=false
Icon=goxlr-utility
Categories=AudioVideo;Audio;Mixer;Utility;
StartupNotify=true
DESKTOP
chmod 0644 "$desktop_target"

if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$app_dir" >/dev/null || true
fi

echo "Installed $binary_target"
echo "Installed $desktop_target"
