# GoXLR Configuration Utility
A tool to configure and use a TC-Helicon GoXLR or GoXLR Mini without requiring windows.

# Project State
As of version 0.2.0 the following features are fully supported:

* Initialisation of Devices under Linux<sup>1</sup>
* Mic and Main Profile Management (Load / Save / New)<sup>2</sup>
* Microphone Selection and Gain
* Fader Assignments
* Fader Mute buttons (Mute channel, Mute to X, Press and Hold)
* The 'Cough' Button (Hold / Toggle, Mute to X, Press and Hold)
* Bleep Button Volume
* Noise Gate and Compressor
* Microphone Equalizer
* Equalizer Fine Tune<sup>3</sup>
* Audio Routing
* Fader and Button colour configurations<sup>3</sup>

<sup>1</sup> Depending on how your GoXLR works, this may require a reboot.  
<sup>2</sup> Profiles are 'cross platform', so Windows profiles should work with the util, and vice versa  
<sup>3</sup> Currently only configurable via the `goxlr-client`

In addition, the Voice Effects panel of the Full GoXLR can be used if loading a pre-configured profile from Windows
(currently not configurable outside of dial adjustments).

# Installation
Currently, the goxlr-utility isn't available via any general distribution method. For now, it must be compiled from
source. As the project develops this will likely change and precompiled binaries will be available to download with
general releases. The following guide should get you started in getting the Utility functional on your system.

## Setting Permissions
Copy `50-goxlr.rules` to `/etc/udev/rules.d/` and then reload with `sudo udevadm control --reload-rules`.

You may need to unplug and replug the GoXLR afterwards, to allow the new permissions to take effect.

## Building from source
### Prerequisites
- Install [Rust](https://rustup.rs/)
- Install libusb
  - Debian/Ubuntu: `apt install libusb-dev`
  - Arch/Manjaro: `pacman -S libusb`
- Have a GoXLR :)

### Building
You can build with `cargo build`, or install the specific executables with:
- `cargo install --path daemon` for the daemon
- `cargo install --path client` for the client to interact with the daemon

Tab-complete files for your terminal of choice will be available after building.

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