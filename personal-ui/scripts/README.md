# Personal UI local production install

This folder contains local-user install helpers for the personal native GoXLR UI. These are intentionally separate from the upstream package metadata so the personal UI can be installed for daily use without changing the system GoXLR Utility package.

## Install or update

From the repo root:

```bash
personal-ui/scripts/install-local.sh
```

The script builds the release binary with the `system-tray` feature and installs:

- `~/.local/bin/goxlr-personal-ui`
- `~/.local/share/applications/goxlr-personal-ui.desktop`

After installation, launch `GoXLR Personal Control` from the desktop/app launcher, or run:

```bash
~/.local/bin/goxlr-personal-ui
```

## Start on login

Install or update the app and add a user autostart entry:

```bash
personal-ui/scripts/install-local.sh --autostart
```

Remove only the autostart entry:

```bash
personal-ui/scripts/install-local.sh --uninstall-autostart
```

The autostart file is written to:

- `~/.config/autostart/goxlr-personal-ui.desktop`

## Fast reinstall from an existing build

```bash
personal-ui/scripts/install-local.sh --no-build
```

## Debug install

```bash
personal-ui/scripts/install-local.sh --debug
```

## Smoke test the local install

```bash
personal-ui/scripts/smoke-local-install.sh
```

This checks the installed binary, desktop entry, optional autostart entry, and runs `desktop-file-validate` when it is available on the system.

## Diagnose runtime dependencies

```bash
personal-ui/scripts/diagnose-runtime.sh
```

This reports common runtime blockers for the installed personal UI: missing binary, missing GoXLR daemon/socket, missing or unreachable PipeWire/PulseAudio through `pactl`, and whether a personal scene config already exists.

## Notes

- The desktop entry uses the existing `goxlr-utility` icon name. If the packaged GoXLR Utility icon is not installed, the launcher may show a generic icon while the app still runs normally.
- This helper only installs the personal UI frontend. It expects the GoXLR daemon/socket to be available through the normal GoXLR Utility setup.
