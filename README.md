# GoXLR configuration utility
A tool to initialize and configure a GoXLR without requiring a Windows VM.

# Warning
This utility is currently not 'user ready', it's extremely rough around the edges and has very little interactivity. You're welcome to experiment with this if you're comfortable working with Rust, but please do not request support at this time.

## Setting Permissions
Copy `50-goxlr.rules` to `/etc/udev/rules.d/` and then reload with `sudo udevadm control --reload-rules`

## Running test demo

For now `cargo build && sudo ./target/debug/goxlr-cli`.

## Running daemon & client

- First build the project with `cargo build`.
- Start the daemon with `sudo ./target/debug/goxlr-daemon`
- Interact with it using `./target/debug/goxlr-client`