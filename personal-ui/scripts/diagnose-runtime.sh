#!/usr/bin/env bash
set -euo pipefail

package_name="goxlr-personal-ui"
bin_path="${XDG_BIN_HOME:-$HOME/.local/bin}/$package_name"
runtime_dir="${XDG_RUNTIME_DIR:-}"
config_home="${XDG_CONFIG_HOME:-$HOME/.config}"

status=0

check() {
  local label="$1"
  shift
  if "$@"; then
    printf 'ok   %s\n' "$label"
  else
    printf 'warn %s\n' "$label"
    status=1
  fi
}

printf 'GoXLR Personal UI runtime diagnostics\n'
printf '======================================\n'

check "installed binary exists and is executable ($bin_path)" test -x "$bin_path"

if [[ -n "$runtime_dir" ]]; then
  printf 'info XDG_RUNTIME_DIR=%s\n' "$runtime_dir"
else
  printf 'warn XDG_RUNTIME_DIR is not set; runtime-dir IPC socket discovery may fail\n'
  status=1
fi

socket_found=false
for candidate in \
  "${runtime_dir:+$runtime_dir/goxlr.socket}" \
  "$HOME/.local/share/goxlr-utility/goxlr.socket" \
  "/tmp/goxlr.socket"; do
  [[ -n "$candidate" ]] || continue
  if [[ -S "$candidate" ]]; then
    printf 'ok   IPC socket exists: %s\n' "$candidate"
    socket_found=true
  else
    printf 'info IPC socket not present: %s\n' "$candidate"
  fi
done

if [[ "$socket_found" != true ]]; then
  printf 'warn no known GoXLR IPC socket found; start or check goxlr-daemon\n'
  status=1
fi

if pgrep -x goxlr-daemon >/dev/null; then
  printf 'ok   goxlr-daemon process is running\n'
else
  printf 'warn goxlr-daemon process not found\n'
  status=1
fi

if command -v pactl >/dev/null; then
  if pactl info >/dev/null 2>&1; then
    printf 'ok   pactl can query PipeWire/PulseAudio\n'
  else
    printf 'warn pactl is installed but cannot query PipeWire/PulseAudio\n'
    status=1
  fi
else
  printf 'warn pactl is not installed; desktop stream routing helpers will be limited\n'
  status=1
fi

if [[ -f "$config_home/goxlr-personal-ui/scenes.json" ]]; then
  printf 'ok   personal scenes config exists: %s\n' "$config_home/goxlr-personal-ui/scenes.json"
else
  printf 'info personal scenes config not found yet; the app can create defaults on first run\n'
fi

exit "$status"
