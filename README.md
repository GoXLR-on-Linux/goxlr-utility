# GoXLR Configuration Utility
A tool to configure and use a TC-Helicon GoXLR or GoXLR Mini without requiring windows.

# Project State
For the current list of features that are supported, as well as what's still left to be done, checkout the 
[Roadmap](ROADMAP.md) in this repository. 

# Installation
Currently, the goxlr-utility isn't available via any general distribution method. For now, it must be compiled from
source. As the project develops this will likely change and precompiled binaries will be available to download with
general releases. The following guide should get you started in getting the Utility functional on your system.

## Setting Permissions
Copy `50-goxlr.rules` to `/etc/udev/rules.d/` and then reload with `sudo udevadm control --reload-rules`.

You **will** need to unplug and replug the GoXLR afterwards, to allow the new permissions to take effect.

## Building from source
### Prerequisites
- Install [Rust](https://rustup.rs/)
- Install libusb
  - Debian/Ubuntu: `apt install libusb-dev`
  - Arch/Manjaro: `pacman -S libusb`
- Have a GoXLR :)

### Building
The easiest way to build is by using the following commands to compile and install the executables:
- `cargo install --path daemon` for the daemon
- `cargo install --path client` for the client to interact with the daemon
Tab-complete files for your terminal of choice will be available after building.

If you'd prefer not to install, you can use `cargo build` and access the binaries in the `target/` directory.

## Running the daemon
You can start the daemon by executing `goxlr-daemon`. The daemon has a couple of possible command line parameters which
can be viewed using `goxlr-daemon --help`.

For the best experience, you should configure `goxlr-daemon` to automatically start on login, for information on how
to achieve this, please consult your distributions' documentation.

If the daemon can't connect to your GoXLR device, check your device permissions (see above!).

## Interacting with the GoXLR
There are two methods for communicating with the daemon and the GoXLR, `goxlr-client` (a command-line configuration
utility), and the embedded [Web UI](https://github.com/GoXLR-on-Linux/goxlr-ui).

### goxlr-client
The GoXLR Client allows for complete configuration of the GoXLR via the command line. This could be useful for
situations where automation of commands and configurations are needed, or you simply don't like the provided UI!

The option list for the CLI is pretty extensive, but reasonably well documented, all parameters, options and their
descriptions can by found via `goxlr-client --help`

### Web UI
Introduced in 0.2.0, the GoXLR daemon now has a web configuration utility available (by default) at 
http://localhost:14564. It's been designed to behave as close to the Windows application as possibly, and as such
should have a familiar design and work as you would expect it to.

If you're not a fan of the WebUI, or would prefer to turn it off, you can start the `goxlr-daemon` process with the
`--http-disable` flag

The UI is still a heavy work in progress, if you'd like to assist, feel free to contribute to its 
[Github Repo](https://github.com/GoXLR-on-Linux/goxlr-ui).

# Disclaimer
This project is also not supported by, or affiliated in any way with, TC-Helicon. For the official GoXLR software, 
please refer to their website.