# GoXLR configuration utility
A tool to initialize and configure a GoXLR without requiring a Windows VM.

## Setting Permissions
Copy `50-goxlr.rules` to `/etc/udev/rules.d/` and then reload with `sudo udevadm control --reload-rules`

## Running

For now `cargo build && sudo ./target/debug/goxlr-cli`. An actual application coming Soon™.
