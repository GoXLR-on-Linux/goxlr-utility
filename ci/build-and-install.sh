#!/bin/bash

# This is a script which will build and install the relevant parts under linux, note that this should
# probably be run under sudo..

D="$(cd "$(dirname "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"

cargo build --release

strip target/release/goxlr-client
strip target/release/goxlr-daemon
strip target/release/goxlr-launcher
strip target/release/goxlr-defaults

DEPLOY_DIR=deployment/out
OUT_DIR_CLIENT="$("$D"/cargo-out-dir target/release/ client-stamp)"
OUT_DIR_DAEMON="$("$D"/cargo-out-dir target/release/ daemon-stamp)"
mkdir -p "$DEPLOY_DIR"

# Copy Shell completions.
cp "$OUT_DIR_DAEMON"/{goxlr-daemon.bash,goxlr-daemon.fish,_goxlr-daemon} "$DEPLOY_DIR/"
cp "$OUT_DIR_CLIENT"/{goxlr-client.bash,goxlr-client.fish,_goxlr-client} "$DEPLOY_DIR/"

sudo cp target/release/goxlr-daemon /usr/bin/
sudo cp target/release/goxlr-client /usr/bin/
sudo cp target/release/goxlr-defaults /usr/bin/
sudo cp target/release/goxlr-launcher /usr/bin/

sudo chmod 755 /usr/bin/goxlr-client /usr/bin/goxlr-daemon /usr/bin/goxlr-defaults /usr/bin/goxlr-launcher

sudo cp daemon/resources/goxlr-utility.png /usr/share/icons/hicolor/48x48/apps/
sudo cp daemon/resources/goxlr-utility.svg /usr/share/icons/hicolor/scalable/apps/
sudo cp daemon/resources/goxlr-utility-large.png /usr/share/pixmaps/goxlr-utility.png
sudo chmod 644 /usr/share/icons/hicolor/48x48/apps/goxlr-utility.png /usr/share/icons/hicolor/scalable/apps/goxlr-utility.svg /usr/share/pixmaps/goxlr-utility.png

sudo cp 50-goxlr.rules /etc/udev/rules.d/
sudo chmod 644 /etc/udev/rules.d/50-goxlr.rules
sudo udevadm control --reload-rules
sudo udevadm trigger

sudo cp deployment/out/goxlr-client.bash /usr/share/bash-completion/completions/
sudo cp deployment/out/goxlr-client.fish /usr/share/fish/vendor_completions.d/
sudo cp deployment/out/_goxlr-client /usr/share/zsh/vendor-completions/
sudo chmod 644 /usr/share/bash-completion/completions/goxlr-client.bash /usr/share/fish/vendor_completions.d/goxlr-client.fish /usr/share/zsh/vendor-completions/_goxlr-client

sudo cp deployment/out/goxlr-daemon.bash /usr/share/bash-completion/completions/
sudo cp deployment/out/goxlr-daemon.fish /usr/share/fish/vendor_completions.d/
sudo cp deployment/out/_goxlr-daemon /usr/share/zsh/vendor-completions/
sudo chmod 644 /usr/share/bash-completion/completions/goxlr-daemon.bash /usr/share/fish/vendor_completions.d/goxlr-daemon.fish /usr/share/zsh/vendor-completions/_goxlr-daemon

echo "Done."