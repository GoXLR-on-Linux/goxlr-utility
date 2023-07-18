[![Support Server](https://img.shields.io/discord/1124010710138106017.svg?label=Discord&logo=Discord&colorB=7289da&style=flat)](https://discord.gg/BRBjkkbvmZ)
[![GitHub tag (latest SemVer pre-release)](https://img.shields.io/github/v/tag/goxlr-on-linux/goxlr-utility?label=Latest)](http://github.com/goxlr-on-linux/goxlr-utility/releases/latest)
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/goxlr-on-linux/goxlr-utility/build.yml)

## GoXLR Configuration Utility
An unofficial tool to configure and control a TC-Helicon GoXLR or GoXLR Mini on Linux, MacOS and Windows. [Click Here](https://discord.gg/BRBjkkbvmZ) to join our discord!

## Features
* Full control over the GoXLR and GoXLR Mini (Similar to the official App)
* Compatibility with profiles created by the official application
* An accessible UI designed to work well with Assistive Technologies
* Remote Access. Control your GoXLR from another computer on your network
* A Sample 'Pre-Buffer'. Record audio from before you press the button
* Exit Actions, including saving profiles and loading other profiles / lighting
* Multiple Device Support. Run more than one GoXLR on one PC
* A CLI and API for basic or advanced scripting and automation
* Streamdeck Integration (through https://github.com/FrostyCoolSlug/goxlr-utility-streamdeck)

## Downloads
Downloads are available on the [Releases Page](https://github.com/GoXLR-on-Linux/goxlr-utility/releases/latest) under the
'Assets' header, we currently provide the following files:

* `.exe` files, usable on Windows<sup>1</sup>
* `.deb` files, usable on Debian based systems (Ubuntu, Mint, Pop!_OS, etc)
* `.rpm` files, usable on Redhat based systems (CentOS, Fedora, etc)

If you're an Arch user, updated versions of the utility are available via [AUR](https://aur.archlinux.org/packages/goxlr-utility)
using the `goxlr-utility` package.  
If you're a MacOS user, check out the [MacOS Project](https://github.com/Adelenade/GoXlr-Macos) for a more integrated implementation.

<sup>1</sup> Windows requires the official device drivers provided by TC-Helicon. If you have the official app 
installed you don't need to do anything, otherwise download the latest drivers from TC-Helicon's website [here](https://go.tc-helicon.com/GoXLR_driver_5.12).

## Getting Started
Once installed, you can launch the Utility using the `GoXLR Utility` item in your Applications Menu, this will launch
the utility and configuration UI. The UI will then be accessible via the system tray icon, or (if you don't have a tray)
by re-running the `GoXLR Utility` menu item.

If you're running on Linux, a first configuration step should be to enable `Autostart on Login` via System -> Settings. 
Windows users will get the choice during installation. If you change your mind, you can change the setting.

If you want to import your profiles from the official app, simply click on the folder icon in the top right of the 
relevant profiles pane (either Main or Mic) which will open the directory in your file browser. Copy the profile across
from the Official App's directory (normally `Documents/GoXLR`) and they'll appear in the util ready to load, simply 
double click them.

If you're setting up from scratch, the best place to start is configuring your microphone. Head over to the `Mic` tab
and hit `Mic Setup` to configure your microphone type and gain. It may be easier to configure if you first set your
Gate Amount to 0, then reconfigure it once your mic is working. Once done, go explore the UI!

## The UI
The Utility's UI is web based, and served directly from the utility to your web browser of choice (if configured, it
can also be served to a web browser on another computer). The UI design was modelled around the official application
in an attempt to provide a familiar interface for those moving from Windows to other platforms, rather than forcing
people to learn a new configuration paradigm.

![image](https://user-images.githubusercontent.com/574943/248385311-0bce92e6-c6c7-4933-81e1-95a36772bb7f.png)

## Building
Build instructions and other useful information can be found on the project's [wiki](https://github.com/GoXLR-on-Linux/goxlr-utility/wiki/Compilation-Guide).
While it's a little sparse at the moment, over time it should grow, and requests / feedback are always welcome!

## Disclaimer
This project is also not supported by, or affiliated in any way with, TC-Helicon. For the official GoXLR software,
please refer to their website.

In addition, this project accepts no responsibility or liability for use of this software, or any problems which may
occur from its use. Please read the [LICENSE](https://github.com/GoXLR-on-Linux/goxlr-utility/blob/main/LICENSE) for
more information.
