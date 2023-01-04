[![Support Server](https://img.shields.io/discord/828348446775574548.svg?label=Discord&logo=Discord&colorB=7289da&style=flat)](https://discord.gg/Wbp3UxkX2j)

# GoXLR Configuration Utility
A tool to configure and use a TC-Helicon GoXLR or GoXLR Mini without requiring windows.

This application assumes you have working audio with your GoXLR device, either out of the box, or via our installation
script. Check out the main [GoXLR on Linux](https://github.com/GoXLR-on-Linux/goxlr-on-linux) repository for info.

# Project State
For the current list of features that are supported, as well as what's still left to be done, checkout the 
[Roadmap](ROADMAP.md) in this repository. 

# Installation
### Linux
We currently provide the following via the [releases page](https://github.com/GoXLR-on-Linux/goxlr-utility/releases/):
* `.deb` files, usable on Debian based systems (Ubuntu, Mint, Pop!_OS, etc)
* `.rpm` files, usable on Redhat based systems (CentOS, Fedora, etc)

We also provide the utility through the [Arch User Repository](https://aur.archlinux.org/packages/goxlr-utility) via the
`goxlr-utility` package.

We've tested as many distributions as possible, but the packages may not work on yours. If you have any problems, 
drop us a message on Discord.

If you require the utility to run on an unsupported distribution, jump to Manual Compilation below.

### MacOS
While the GoXLR Utility does work correctly under MacOS, it comes with several caveats. The following needs to be 
considered:

* It must be manually compiled (we are currently not producing builds)
* Aggregate devices must be created to split the GoXLRs multichannel input / output to usable channels
* When a device is attached, the `goxlr-initialiser` binary needs to be run with sudo to prepare the device
* There is no 'auto start' functionality, and the `goxlr-launcher` binary will not launch the daemon.

We may look into directly solving these problems down the line, but in the meantime a community member has been working
on them over at the [GoXLR MacOS](https://github.com/Adelenade/GoXlr-Macos) project, as well as providing a more
integrated swift based UI, so if you're on Mac, check that out!

### Windows
The GoXLR Utility is usable under Windows through the official TC-Helicon drivers. The utility installer is available
via the [releases page](https://github.com/GoXLR-on-Linux/goxlr-utility/releases/). There are a couple of things to 
note:

* Windows support is relatively new, so may not be as stable.
* The official GoXLR driver is required, and the util has been tested against version 5.12 (Available from TC-Helicon 
[here](https://go.tc-helicon.com/GoXLR_driver_5.12))
* The driver *MUST* be installed to the default location on drive C.
* To prevent conflicts, the utility will abort and quit if it detects the Official GoXLR App running

If you have any problems, be sure to use our discord, and not the official one! In addition, a huge shout-out to
oddbear for doing most the heavy lifting in getting the utility functional under Windows, and to TC-Helicon for
graciously permitting us to release it! 

# Usage
## Running the utility
If you installed the utility via a package, there should be a 'GoXLR Utility' item in your application list. This will
start the background daemon (if needed), then launch the UI for configuration. The background Daemon will then continue
running until you log out or quit (this is required for the GoXLR to function). If your desktop environment supports a
system tray, an icon will appear there for quick opening the configuration.

If you're manually compiling, the produced `goxlr-launcher` binary will do the above.

### Automatically Start on Login
The GoXLR Utility supports automatically starting on login, from the UI, go to `System` -> `Settings`, and tick the
`Autostart on Login` box.

### Advanced Running
For more control over the GoXLR utility, the `goxlr-daemon` can be manually launched with various configuration
settings. For more information, check out `goxlr-daemon --help`. Note that any extra parameters passed to the
daemon will not be propagated into autostart if you enable it (You'll need to manually edit
`~/.config/autostart/goxlr-daemon.desktop` once autostart is enabled).

## Interacting with the GoXLR
For most people, running the GoXLR Utility from your menu or interacting with the system tray icon will bring up
the web configuration UI allowing you to fully configure the GoXLR from your web browser. If you'd like to help improve
the web experience, check out the [UI Repository](https://github.com/GoXLR-on-Linux/goxlr-ui) and get tweaking!

There is also a command line client, `goxlr-client`, which can be used to configure all parts of the GoXLR without
the main UI. This can be useful for automation scenarios (for example, changing colour themes when locking / unlocking
your computer), setting up keyboard shortcuts, or even additional Stream Deck actions. For a list of all parameters and
options (there are a lot!) check out `goxlr-client --help`.

# The GoXLR Utility API
The GoXLR Utility is an API driven application, which allows third party programs and applications to communicate with,
monitor, and make changes to the GoXLR. The WebUI and `goxlr-client` binaries are examples of API clients. If
you're interested in building a tool with the API, check out [This Wiki Page](https://github.com/GoXLR-on-Linux/goxlr-utility/wiki/The-GoXLR-Utility-API)
which will contain details.

# Manual Compilation
## Setting Permissions
Copy `50-goxlr.rules` to `/etc/udev/rules.d/` and then reload with `sudo udevadm control --reload-rules && sudo udevadm trigger`.

## Building from source
### Prerequisites
* Have a GoXLR :)
* Install [Rust](https://rustup.rs/)
* For Linux:
  * Debian: `sudo apt-get install pkg-config libdbus-1-dev libpulse0`
  * Fedora: `sudo dnf install pkgconf-pkg-config dbus-devel pulseaudio-libs`


### Building
The easiest way to build is by using the following commands to compile and install the executables:
- `cargo install --path daemon` for the daemon
- `cargo install --path client` for the client to interact with the daemon
- `cargo install --path defaults` for the Default Profile Handlers
- `cargo install --path launcher` for the Utility Launcher

Tab-complete files for your terminal of choice will be available after building.

If you'd prefer not to install, you can use `cargo build` and access the binaries in the `target/` directory.

# Disclaimer
This project is also not supported by, or affiliated in any way with, TC-Helicon. For the official GoXLR software, 
please refer to their website.
