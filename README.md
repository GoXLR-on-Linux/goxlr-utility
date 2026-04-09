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
* Streamdeck Integration (
  through [The StreamDeck Repository](https://github.com/FrostyCoolSlug/goxlr-utility-streamdeck))

## Downloads

Downloads are available on the [Releases Page](https://github.com/GoXLR-on-Linux/goxlr-utility/releases/latest) under
the
'Assets' header, we currently provide the following files:

* `.exe` files, usable on Windows<sup>1</sup>
* `.pkg` files, usable on MacOS, both Intel and M1 based packages are available<sup>2</sup>
* `.deb` files, usable on Debian based systems (Ubuntu, Mint, Pop!_OS, etc)
* `.rpm` files, usable on Redhat based systems (CentOS, Fedora, etc)

### OS / Distro Specific Notes

* If you are running Ubuntu 24.04 or a derivitive (such as Linux Mint), please review
  [this issue](https://github.com/GoXLR-on-Linux/goxlr-utility/issues/221)
* If you're running the Mix 2 firmware and are seeing UCM errors, please
  review [this issue](https://github.com/GoXLR-on-Linux/goxlr-utility/issues/223)
* Arch users can install the `goxlr-utility` package from [AUR](https://aur.archlinux.org/packages/goxlr-utility)
* Fedora Atomic or Bazzite users please check the instructions
  [here](https://github.com/GoXLR-on-Linux/goxlr-utility/wiki/Fedora-Atomic-&-Bazzite)
* Windows users can also aquire the GoXLR Utility via `winget`

<sup>1</sup> Windows requires the official device drivers provided by TC-Helicon. If you have the official app
installed you don't need to do anything, otherwise download the latest drivers from TC-Helicon's
website [here](https://mediadl.musictribe.com/media/PLM/sftp/incoming/hybris/import/goxlr/driverRepair/TC-Helicon_GoXLR_Driver.zip).

<sup>2</sup> MacOS support is still somewhat experimental, and the package may conflict with the existing
GoXLR-MacOS project as they attempt to do the same thing in certain situations.

## Integrations

* [twitchat](https://twitchat.fr/) - Activate and change GoXLR settings based on twitch bits / donations (Thanks Durss!)
* [MacroGraph](https://www.macrograph.app/) - A visual programmer for Streamers. (Thanks JDUDE!)
* [OBS Fader Sync](https://github.com/parzival-space/obs-goxlr-fader-sync-plugin) - An OBS plugin to sync pre-mix
  volumes to fader volumes (Thanks parzival!)
* [Home Assistant](https://github.com/timmo001/homeassistant-integration-goxlr-utility) - A plugin that lets you tie the
  GoXLR into your home automation (Thanks timmmo!)

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

The Utility's local UI is launched through the GoXLR UI desktop application (`goxlr-utility-ui`), which wraps the web
interface into a dedicated app window. The UI design was modelled around the official application in an attempt to
provide a familiar interface for those moving from Windows to other platforms, rather than forcing people to learn a
new configuration paradigm.

![image](https://github.com/GoXLR-on-Linux/goxlr-utility/assets/574943/8f14bd2c-e67a-42e5-bd9f-b3cb367e171d)

If you're running on Linux, the UI application isn't provided as part of the base utility installation. Install it from
the [GoXLR UI Repository](https://github.com/frostyCoolSlug/goxlr-utility-ui/), which provides builds for multiple
distributions.

## API Security

The daemon's HTTP API can be protected with an optional bearer token.

Set `GOXLR_HTTP_TOKEN` before starting `goxlr-daemon`:

```bash
export GOXLR_HTTP_TOKEN="replace-with-a-strong-token"
goxlr-daemon
```

When set, these endpoints require authentication:

* `POST /api/command`
* `GET /api/get-devices`
* `GET /api/path`
* `GET /files/scribble/{serial}/{fader}.png`
* `GET /files/samples/{sample}`
* `POST /firmware-upload/{serial}`
* `GET /api/websocket`

Token can be provided as either:

* `Authorization: Bearer <token>`
* `?token=<token>` query parameter (useful for websocket clients)

Example request:

```bash
curl -H "Authorization: Bearer replace-with-a-strong-token" \
  http://127.0.0.1:14564/api/get-devices
```

## Local Change Log

This section tracks local source changes made on this machine so context survives restarts.

### 2026-04-09

* Added Linux startup preflight checks in daemon startup path:
  * GoXLR udev rule presence
  * USB power policy check (`power/control=on`)
  * PipeWire/WirePlumber session checks
  * ALSA split.conf compatibility check and auto-patch when writable
* Added ALSA UCM compatibility patching to packaging scripts:
  * Debian `postinst` script updates `/usr/share/alsa/ucm2/common/pcm/split.conf`
  * RPM `post_install_script` includes the same fix
* Hardened runtime crash paths:
  * Removed panic path in firmware update state handling
  * Removed `unwrap()` panic in sample file HTTP serving
  * Removed multiple file watcher/glob `unwrap()` panic paths
* Improved settings save safety:
  * On Unix, settings write now avoids delete-before-rename window
* Hardened IPC socket isolation:
  * Socket path now uses per-user runtime path:
    * Unix: `$XDG_RUNTIME_DIR/goxlr.socket` (fallback `/tmp/goxlr-<uid>.socket`)
    * Windows: `@goxlr.socket`
  * Applied to daemon, launcher, and client
* Added optional HTTP API authentication token:
  * `GOXLR_HTTP_TOKEN` enables auth
  * Protected endpoints require `Authorization: Bearer <token>` or `?token=<token>`
* Changed UI launch behavior to app-first and then app-only:
  * Activate path now prefers native UI app (`goxlr-utility-ui`) when available
  * Browser fallback removed from UI activation path
  * `OpenUi` command is now a compatibility alias of `Activate`
* Hardened UI launch command parsing on Unix:
  * Empty `activate` command values now fall back safely to detected UI app instead of indexing an empty arg list
* Hardened daemon and launcher binary discovery:
  * Removed `unwrap()` usage on `current_dir()` / `current_exe()` path discovery for UI and daemon binaries
* Enforced app-only UI activation flow:
  * Daemon activate event now always launches detected `goxlr-utility-ui`
  * `SetActivatorPath` daemon command now normalizes to detected app path
  * Daemon status reports app path as active UI handler path
* Updated bundled web settings UI handler options:
  * Browser and custom UI handler choices removed from bundled web asset
  * UI handler selector now only exposes `app`
* Improved launcher reliability when opening UI:
  * Added IPC connection retry loop before failing activation request
* Added on-demand Linux health check trigger:
  * New tray menu action `Run Health Check` reruns daemon preflight checks/fixes without restart
  * Preflight failures are surfaced via platform error dialog and warning logs
* Secured media file HTTP endpoints with auth:
  * `GET /files/scribble/{serial}/{fader}.png` now requires token when `GOXLR_HTTP_TOKEN` is set
  * `GET /files/samples/{sample}` now requires token when `GOXLR_HTTP_TOKEN` is set
* Removed legacy `OpenUi` daemon command:
  * IPC `DaemonCommand` now uses `Activate` only for UI launch
* Added startup UI-missing notification:
  * When auto-launch UI is requested and `goxlr-utility-ui` is missing, daemon now shows a platform error dialog
* Updated app-only wording in docs and in-app English UI text:
  * README `The UI` section now describes desktop app launch flow
  * Bundled English settings text now labels this as `UI Application` / `Desktop App`
* Added README API security documentation for `GOXLR_HTTP_TOKEN`
* Added new high-impact audio safety feature commands:
  * `ClipGuard` device setting with configurable threshold (IPC + CLI)
  * Headphone limiter setting with configurable threshold (IPC + CLI)
  * Volume cap enforcement now applies to direct volume, submix volume, and hardware fader updates
* Added mic wizard groundwork command path:
  * IPC clients now support fetching live mic input level
  * CLI command `microphone input-level` prints current mic level in dB
* Added GUI controls for new audio safety features in Device Settings modal:
  * `ClipGuard` toggle + threshold slider (0-100%)
  * `Headphone Limiter` toggle + threshold slider (0-100%)
  * Controls are wired to `SetClipGuardEnabled/Threshold` and `SetHeadphoneLimiterEnabled/Threshold`
* Improved Mic Setup wizard guidance in bundled GUI:
  * Mic waveform now shows live level text (`Live: <dB>`) during setup
  * Added target-zone hint (`Target: -20 to -10 dB`) directly on the meter
  * Enabled both estimated-noise and live guidance overlays in Mic Setup flow
* Polished bundled GUI labels/descriptions for new audio safety controls:
  * Clarified ClipGuard and Headphone Limiter wording in Device Settings
  * Added practical threshold guidance text (`ClipGuard` start at `95%`, headphone limiter around `85-90%`)
  * Renamed `Headphone Limit Threshold` to `Headphone Limiter Threshold`
* Added one-click audio safety presets in Device Settings GUI:
  * New buttons: `Safe`, `Balanced`, `Loud`
  * Presets apply both ClipGuard and Headphone Limiter enable states and thresholds
  * Default mapping: `Safe` (90/80), `Balanced` (95/88), `Loud` (98/94)
* Added active preset highlighting for audio safety buttons:
  * GUI now auto-detects when thresholds match `Safe`, `Balanced`, or `Loud`
  * Matching preset button is shown as active and marked with `aria-pressed`

### 2026-04-10

* Added explicit `Custom` indicator for audio safety presets in Device Settings GUI:
  * `Custom` label appears when current ClipGuard / Headphone Limiter values do not match `Safe`, `Balanced`, or `Loud`
  * Label auto-hides when a preset match is detected
* Added headphone-focused quick actions in Device Settings GUI:
  * `Night Listening` button: enables headphone limiter, sets limiter threshold to `80%`, and sets Headphones volume to `70%`
  * `Music Listening` button: enables headphone limiter, sets limiter threshold to `90%`, and sets Headphones volume to `85%`
  * Implemented using existing GUI command path (`SetHeadphoneLimiterEnabled`, `SetHeadphoneLimiterThreshold`, `SetVolume`)
* Added Linux headphone EQ backend integration and GUI controls:
  * New per-device EQ settings + profiles (`Flat`, `Music`, `Voice`, `Night`) with save/load commands
  * Backend writes and loads dedicated EasyEffects output preset (`GoXLR-HeadphoneEQ-<serial>`)
  * Device Settings now includes `Headphone EQ` enable toggle, preamp control, Bass/Mid/Treble quick adjusters, and profile save/load buttons
  * Requires `easyeffects` to be installed on Linux for active DSP processing
* Hardened EasyEffects auto-load behavior for headphone EQ:
  * Backend now tries direct preset load first, then retries after hidden EasyEffects startup
  * If session has no active display, daemon keeps settings/preset write successful and logs warning instead of hard-failing EQ updates
* Added full headphone EQ command surface to CLI:
  * New `settings headphone-eq` subcommands for enable, preamp, per-band gain/frequency/Q, and save/load/delete profile
  * Added strict CLI range validation for parametric EQ values (band index, gain, frequency, and Q)
* Expanded bundled Device Settings Headphone EQ GUI to full multi-band controls:
  * Added per-band Gain/Frequency/Q controls for all available bands (default 10)
  * Added active profile indicator and dynamic load/save buttons for detected profile names
  * Added `Save As...` and `Delete Profile...` actions directly in the modal

### Logging Rule

For every future functional change, add a short dated bullet under this section in the same format.

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
