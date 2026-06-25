#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/../.." && pwd)"
package_name="goxlr-personal-ui"
bin_path="${XDG_BIN_HOME:-$HOME/.local/bin}/$package_name"
desktop_path="${XDG_DATA_HOME:-$HOME/.local/share}/applications/$package_name.desktop"
autostart_path="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/$package_name.desktop"

cd "$repo_root"

bash -n personal-ui/scripts/install-local.sh

if [[ ! -x "$bin_path" ]]; then
  echo "Missing installed executable: $bin_path" >&2
  echo "Run personal-ui/scripts/install-local.sh first." >&2
  exit 1
fi

if [[ ! -f "$desktop_path" ]]; then
  echo "Missing desktop launcher: $desktop_path" >&2
  echo "Run personal-ui/scripts/install-local.sh first." >&2
  exit 1
fi

python3 - <<PY
from pathlib import Path
import os
bin_path = Path(${bin_path@Q})
desktop_path = Path(${desktop_path@Q})
autostart_path = Path(${autostart_path@Q})
if not bin_path.exists() or not os.access(bin_path, os.X_OK):
    raise SystemExit(f'Installed binary missing or not executable: {bin_path}')
text = desktop_path.read_text()
for needle in [
    '[Desktop Entry]',
    'Type=Application',
    'Name=GoXLR Personal Control',
    f'Exec={bin_path}',
    'Terminal=false',
    'Icon=goxlr-utility',
    'Categories=AudioVideo;Audio;Mixer;',
    'Keywords=GoXLR;Mixer;Audio;Microphone;Streaming;',
    'StartupNotify=true',
]:
    if needle not in text:
        raise SystemExit(f'Desktop launcher missing {needle!r}')
if autostart_path.exists():
    autostart = autostart_path.read_text()
    for needle in [f'Exec={bin_path}', 'X-GNOME-Autostart-enabled=true']:
        if needle not in autostart:
            raise SystemExit(f'Autostart launcher missing {needle!r}')
print('local install smoke assertions passed')
PY

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$desktop_path"
  if [[ -f "$autostart_path" ]]; then
    desktop-file-validate "$autostart_path"
  fi
else
  echo "desktop-file-validate not installed; skipped desktop metadata validation"
fi

echo "Installed binary: $bin_path"
echo "Desktop launcher: $desktop_path"
if [[ -f "$autostart_path" ]]; then
  echo "Autostart launcher: $autostart_path"
else
  echo "Autostart launcher: not installed"
fi
