[![Support Server](https://img.shields.io/discord/1124010710138106017.svg?label=Discord&logo=Discord&colorB=7289da&style=flat)](https://discord.gg/BRBjkkbvmZ)
[![GitHub tag (latest SemVer pre-release)](https://img.shields.io/github/v/tag/goxlr-on-linux/goxlr-utility?label=Latest)](http://github.com/goxlr-on-linux/goxlr-utility/releases/latest)
![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/goxlr-on-linux/goxlr-utility/build.yml)

## GoXLR Configuration Utility

An unofficial tool to configure and control a TC-Helicon GoXLR or GoXLR Mini on Linux, MacOS and
Windows. [Click Here](https://discord.gg/BRBjkkbvmZ) to join our discord!

## Features

* Full control over the GoXLR and GoXLR Mini (Similar to the official App)
* Compatibility with profiles created by the official application
* An accessible UI designed to work well with Assistive Technologies
* Remote Access. Control your GoXLR from another computer on your network
* A Sample 'Pre-Buffer'. Record audio from before you press the button
* Exit Actions, including saving profiles and loading other profiles / lighting
* Multiple Device Support. Run more than one GoXLR on one PC
* A CLI and API for basic or advanced scripting and automation
* Streamdeck Integration (through [The StreamDeck Repository](https://github.com/FrostyCoolSlug/goxlr-utility-streamdeck))

## Downloads

Downloads are available on the [Releases Page](https://github.com/GoXLR-on-Linux/goxlr-utility/releases/latest) under
the
'Assets' header, we currently provide the following files:

* `.exe` files, usable on Windows<sup>1</sup>
* `.pkg` files, usable on MacOS, both Intel and M1 based packages are available<sup>2</sup>
* `.deb` files, usable on Debian based systems (Ubuntu, Mint, Pop!_OS, etc)
* `.rpm` files, usable on Redhat based systems (CentOS, Fedora, etc)

If you're an Arch user, updated versions of the utility are available
via [AUR](https://aur.archlinux.org/packages/goxlr-utility)
using the `goxlr-utility` package.

The GoXLR Utility is also available via `winget` on Windows, and will automatically update when new releases occur.

<sup>1</sup> Windows requires the official device drivers provided by TC-Helicon. If you have the official app
installed you don't need to do anything, otherwise download the latest drivers from TC-Helicon's
website [here](https://mediadl.musictribe.com/download/software/tchelicon/GoXLR/TC-Helicon_GoXLR_Driver.zip).

<sup>2</sup> MacOS support is still somewhat experimental, and the package may conflict with the existing
GoXLR-MacOS project as they attempt to do the same thing in certain situations.

## Integrations
* [twitchat](https://twitchat.fr/) - Activate and change GoXLR settings based on twitch bits / donations (Thanks Durss!)
* [OBS Fader Sync](https://github.com/parzival-space/obs-goxlr-fader-sync-plugin) - An OBS plugin to sync pre-mix volumes to fader volumes (Thanks parzival!)
* [Home Assistant](https://github.com/timmo001/homeassistant-integration-goxlr-utility) - A plugin that lets you tie the GoXLR into your home automation (Thanks timmmo!)

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

The Utility's UI is web based and served directly from the utility to your web browser of choice (if configured, it
can also be served to a web browser on another computer). The Utility also provides an 'Application' which wraps the
web UI into a dedicated app. If you're using the Utility on Windows this option is presented to you during install.
The UI design was modelled around the official application in an attempt to provide a familiar interface for those
moving from Windows to other platforms, rather than forcing people to learn a new configuration paradigm.

![image](https://github.com/GoXLR-on-Linux/goxlr-utility/assets/574943/8f14bd2c-e67a-42e5-bd9f-b3cb367e171d)

If you're running on Linux, the 'Application' isn't provided as part of the base utility installation. If you'd
prefer to use it, check out the [GoXLR UI Repository](https://github.com/frostyCoolSlug/goxlr-utility-ui/), which
provides various builds for distributions. Once installed, you should be able to go to System -> Utility Settings 
and change the UI Handler there.


## Building

Build instructions and other useful information can be found on the
project's [wiki](https://github.com/GoXLR-on-Linux/goxlr-utility/wiki/Compilation-Guide).
While it's a little sparse at the moment, over time it should grow, and requests / feedback are always welcome!

## Disclaimer

This project is also not supported by, or affiliated in any way with, TC-Helicon. For the official GoXLR software,
please refer to their website.

In addition, this project accepts no responsibility or liability for use of this software, or any problems which may
occur from its use. Please read the [LICENSE](https://github.com/GoXLR-on-Linux/goxlr-utility/blob/main/LICENSE) for
more information.
