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

## Fast reinstall from an existing build

```bash
personal-ui/scripts/install-local.sh --no-build
```

## Debug install

```bash
personal-ui/scripts/install-local.sh --debug
```

## Notes

- The desktop entry uses the existing `goxlr-utility` icon name. If the packaged GoXLR Utility icon is not installed, the launcher may show a generic icon while the app still runs normally.
- This helper only installs the personal UI frontend. It expects the GoXLR daemon/socket to be available through the normal GoXLR Utility setup.
