#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Install the GoXLR personal native UI into the current user's local desktop session.

Usage: personal-ui/scripts/install-local.sh [--no-build] [--debug] [--autostart|--no-autostart|--uninstall-autostart]

Options:
  --no-build             Reuse an existing target binary instead of building first.
  --debug                Install target/debug/goxlr-personal-ui instead of target/release/goxlr-personal-ui.
  --autostart            Also install a user autostart entry for login startup.
  --no-autostart         Install/update the app launcher only; leave any existing autostart entry untouched.
  --uninstall-autostart  Remove the user autostart entry and exit without building or reinstalling the app.
  -h, --help             Show this help.

Installs:
  ~/.local/bin/goxlr-personal-ui
  ~/.local/share/applications/goxlr-personal-ui.desktop
  ~/.config/autostart/goxlr-personal-ui.desktop  (only with --autostart)
USAGE
}

build=true
profile=release
autostart_action=leave

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-build)
      build=false
      ;;
    --debug)
      profile=debug
      ;;
    --autostart)
      autostart_action=install
      ;;
    --no-autostart)
      autostart_action=leave
      ;;
    --uninstall-autostart)
      autostart_action=remove
      build=false
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
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"
app_dir="$data_home/applications"
autostart_dir="$config_home/autostart"
binary_target="$bin_dir/$package_name"
desktop_target="$app_dir/$package_name.desktop"
autostart_target="$autostart_dir/$package_name.desktop"

write_desktop_entry() {
  local target="$1"
  local autostart_enabled="${2:-false}"
  cat > "$target" <<DESKTOP
[Desktop Entry]
Type=Application
Name=GoXLR Personal Control
GenericName=GoXLR Mixer Control
Comment=Personal native Rust control panel for GoXLR Utility
Exec=$binary_target
Terminal=false
Icon=goxlr-utility
Categories=AudioVideo;Audio;Mixer;
Keywords=GoXLR;Mixer;Audio;Microphone;Streaming;
StartupNotify=true
X-GNOME-Autostart-enabled=$autostart_enabled
DESKTOP
  chmod 0644 "$target"
}

if [[ "$autostart_action" == "remove" ]]; then
  rm -f "$autostart_target"
  echo "Removed $autostart_target"
  exit 0
fi

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
write_desktop_entry "$desktop_target" false

if [[ "$autostart_action" == "install" ]]; then
  mkdir -p "$autostart_dir"
  write_desktop_entry "$autostart_target" true
fi

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$desktop_target"
  if [[ "$autostart_action" == "install" ]]; then
    desktop-file-validate "$autostart_target"
  fi
fi

if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$app_dir" >/dev/null || true
fi

echo "Installed $binary_target"
echo "Installed $desktop_target"
if [[ "$autostart_action" == "install" ]]; then
  echo "Installed $autostart_target"
else
  echo "Autostart unchanged; rerun with --autostart to enable login startup."
fi
