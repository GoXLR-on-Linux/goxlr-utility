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

* Bearer authorization header for ordinary HTTP requests
* `?token=<token>` query parameter for `/api/websocket` only (browser websocket clients cannot reliably set `Authorization` headers)
* `/?token=<token>` once in the bundled browser UI, which sets an HttpOnly same-origin session cookie and redirects back to `/`

Example request:

```bash
curl -H "Authorization: Bearer replace-with-a-strong-token" \
  http://127.0.0.1:14564/api/get-devices
```

## Local Change Log

This section tracks local source changes made on this machine so context survives restarts.

* Fixed CI failures on the personal UI production launcher branch:
  * Updated the personal UI tray dependency to `ksni` 0.3.5 and switched the feature selection to the crate's default Tokio backend plus `blocking`, avoiding the CI `--all-features` conflict between `tokio` and `async-io`
  * Addressed Clippy `-D warnings` findings in `personal-ui/src/lib.rs`: derived `MiniWindowMode::Default`, boxed the large `WorkerEvent::Snapshot` variant, collapsed flagged nested conditionals, and removed cross-platform unused mutability from IPC socket candidates
  * Verified the personal UI package locally with formatting, all-feature package check, full app-model tests, and the system-tray feature check; workspace-wide `cargo check --all-features` remains blocked locally by missing `libclang` for the unrelated `speech-dispatcher-sys` build script

* Expanded personal UI local production helpers:
  * Added optional `--autostart` / `--uninstall-autostart` support to `personal-ui/scripts/install-local.sh`, writing a user login entry at `~/.config/autostart/goxlr-personal-ui.desktop`
  * Added `personal-ui/scripts/smoke-local-install.sh` to verify the installed binary, desktop entry, optional autostart entry, and desktop metadata validation when `desktop-file-validate` is installed
  * Added `personal-ui/scripts/diagnose-runtime.sh` to surface common installed-runtime blockers such as missing daemon/socket, missing installed binary, and unavailable `pactl` / PipeWire-Pulse routing support
  * Polished desktop metadata with `GenericName`, `Keywords`, and `X-GNOME-Autostart-enabled` fields while keeping the existing `goxlr-utility` icon

* Started a personal UI local production launcher/install chunk:
  * Added `personal-ui/scripts/install-local.sh` to build the native personal UI with the `system-tray` feature and install it into the current user's `~/.local/bin`
  * The script writes a desktop launcher at `~/.local/share/applications/goxlr-personal-ui.desktop` so the app can be started as `GoXLR Personal Control` from the desktop menu
  * Added `personal-ui/scripts/README.md` documenting install, no-build reinstall, debug install, and daemon/socket expectations

* Added a production-readiness stabilization pass for the current personal UI chunk:
  * Updated `personal-ui/MANUAL_QA.md` with the current running app session/PID, production-use posture, tool blockers for `cargo clippy` / `cargo audit`, and the remaining manual screenshot gate
  * Refreshed `personal-ui/PARITY_CHECKLIST.md` recommended next choices to prioritize one final hands-on pass and commit over stacking more feature work
  * Kept intentionally partial, higher-risk areas deferred instead of broadening scope while preparing the current safe chunk for production use

* Added hardware-first headphone listening presets to the native Headphone EQ page:
  * Added `HeadphoneListeningPreset` model bundles for `Neutral Base`, `Music Detail`, `Game Imaging`, and `Night Safe`
  * Each preset deliberately sets the monitored output to Headphones, headphone volume, limiter enable/threshold, EQ enable/preamp, and ten-band hardware EQ gain/frequency/Q values rather than relying on the original web UI feature set
  * Added a `HEADPHONE EXPERIENCE` panel with a five-step hardware-first tuning order so headphone work starts with routing, gain staging, limiter safety, purpose-specific EQ, and profile save discipline

* Added direct Fader A-D mute-state controls to the native Mixer dashboard:
  * Added `FaderMuteStateControl` model helpers and a compact `Current state` button row in each Mixer fader card for `Unmute`, `Mute target`, and `Mute all`
  * Added typed `PersonalCommand::SetFaderMuteState` mapping to the existing daemon `GoXLRCommand::SetFaderMuteState`
  * Kept hold/toggle behaviour and less-used mute targets deferred; this chunk only exposes deliberate current-state actions for the daily Fader A-D cards

* Cleaned stale native parity checklist guidance after the Mixer submix slider pass:
  * Refreshed the checklist date and recommended next choices to prioritize committing the verified Mixer chunk before stacking more code
  * Updated Mixer, Effects, Lighting, and Sampler priority notes so remaining work is framed as scoped edge cases, profile/file workflows, or screenshot-driven polish instead of broad missing parity
  * Left genuinely partial manager/editor rows as `[~]` where free-form import/rename, waveform/drag-drop, hidden Megaphone parameters, or exhaustive icon/output coverage remain deferred

* Added arbitrary Mixer submix volume sliders to the native Mixer dashboard:
  * Added `SubmixVolumeSlider` model helpers for Mic, Chat, Music, Game, Console, Line In, System, and Sample custom 0–100% channel volumes
  * Rendered a `CUSTOM CHANNEL VOLUMES` section in the bounded `SUBMIX` panel, seeded from daemon-reported submix state when available
  * Clamped percent values and converted them to the backend raw 0–255 `SetSubMixVolume` scale before dispatching typed commands

* Added arbitrary clamped Pitch and Megaphone DSP sliders to the native Effects page:
  * Added `EffectsPitchSlider` for pitch character and `EffectsMegaphoneSlider` for megaphone post gain
  * Rendered `PITCH SLIDERS` and `MEGAPHONE SLIDERS` sections in the Effects `ADVANCED DSP` panel alongside the existing Reverb/Echo/Robot/Hard Tune sections
  * Clamped slider input to safe typed ranges and dispatched existing `SetPitchCharacter` / `SetMegaphonePostGain` command mappings

* Added arbitrary clamped Robot and Hard Tune DSP sliders to the native Effects page:
  * Added `EffectsRobotSlider` model helpers for low/mid/high gain, frequency, and width bands plus waveform, pulse width, threshold, and dry mix
  * Added `EffectsHardTuneSlider` model helpers for amount, rate, and window
  * Rendered `ROBOT SLIDERS` and `HARD TUNE SLIDERS` sections in the Effects `ADVANCED DSP` panel, with clamped typed `SetRobot*` / `SetHardTune*` command dispatch

* Added arbitrary clamped Echo DSP sliders to the native Effects page:
  * Added `EffectsEchoSlider` model helpers for feedback, tempo, left/right delays, left/right feedback, and cross-feedback in both directions
  * Rendered an `ECHO SLIDERS` section in the Effects `ADVANCED DSP` panel next to the existing Reverb sliders and quick-default buttons
  * Clamped slider input to safe typed ranges and dispatched existing `SetEcho*` `PersonalCommand` / `GoXLRCommand` mappings

* Added arbitrary clamped Reverb DSP sliders to the native Effects page:
  * Added `EffectsReverbSlider` model helpers for decay, early/tail levels, pre-delay, low/high colour, high factor, diffuse, and modulation speed/depth
  * Rendered a `REVERB SLIDERS` section in the Effects `ADVANCED DSP` panel before the existing quick-default buttons
  * Clamped slider input to safe typed ranges and dispatched existing `SetReverb*` `PersonalCommand` / `GoXLRCommand` mappings; Echo/Robot/Hard Tune arbitrary sliders remain separate future chunks

* Added live Sampler custom start/stop trim sliders:
  * Added `SampleTrimEditor` model helpers that clamp arbitrary percentages to 0–100 and emit typed `SetSampleStartPercent` / `SetSampleStopPercent` commands for the daemon-reported bank/pad/sample index
  * Rendered Start and Stop sliders beside each live sample row, so per-index trim no longer depends only on preset boundary buttons
  * Kept trim editing tied to live daemon sample rows; waveform visualization, drag/drop, and bulk sample workflows remain deferred

* Added a discovered Lighting profile browser for colour-only loads:
  * Added `ProfileBrowserKind::LightingColours` so the Lighting page can list `.goxlr` profiles and send guarded per-row `LoadProfileColours` actions
  * Reused the existing same-action second-click confirmation flow and separate lighting-profile pending state instead of adding destructive profile management
  * Kept the older named-slot lighting button as a quick action while adding discovered profile rows for real local profiles

* Expanded safe Sampler start/stop trim presets:
  * Broadened `SampleTrimAction::safe_trim_actions()` from reset-only `Start 0%` / `Stop 100%` buttons to bounded start presets at 0%, 25%, and 50% plus stop presets at 50%, 75%, and 100%
  * Kept trim editing tied to daemon-backed live sample rows and existing typed `SetSampleStartPercent` / `SetSampleStopPercent` commands rather than adding arbitrary percentage inputs
  * Updated focused Sampler model coverage for labels, target bank/pad/index, and representative start/stop percentages

* Expanded native Lighting individual button colour/off-style coverage:
  * Broadened `LightingButtonColourTarget::daily_targets()` from button groups plus Cough/Bleep to 20 safe targets covering effect preset buttons, FX/Megaphone/Robot/Hard Tune buttons, and sampler pad/clear buttons
  * Kept the workflow model-backed through existing typed `SetButtonColours` and `SetButtonOffStyle` commands; no arbitrary/raw lighting command editor was added
  * Updated focused Lighting model coverage for target count plus representative effect and sampler button command mappings

* Added read-only live System settings status rows to the native System page:
  * Added `SystemSettingsSnapshot` and `SystemSettingsStatusRow` model helpers for daemon-reported mute hold, VC/chat mic coupling, monitor-with-FX, fader lock, and VOD mode
  * Populated the snapshot from daemon mixer settings and rendered a `LIVE SYSTEM SETTINGS` panel above the explicit action buttons
  * Kept this status panel read-only; all setting changes still go through the existing typed preset buttons

* Added safe cough-button behaviour controls to the native System page:
  * Added typed `PersonalCommand::SetCoughIsHold` and `PersonalCommand::SetCoughMuteFunction` mappings to daemon `GoXLRCommand`
  * Added daily buttons for cough hold/toggle mode plus All, Stream, Voice Chat, and Phones mute targets
  * Kept the workflow as explicit button presets rather than arbitrary/raw device setting edits

* Added live daemon-backed submix state reflection to the native Mixer dashboard:
  * Added `SubmixChannelSnapshot` and `SubmixOutputSnapshot` model helpers for current channel volume/link state and output Mix A/B state
  * Populated submix snapshots from daemon `levels.submix` when available, while leaving the existing safe preset/link/output buttons unchanged
  * Rendered inline current submix state beside channel and output controls; arbitrary sliders and exhaustive outputs remain deferred

* Added a read-only recent app/IPC event log to the native Diagnostics / Status page:
  * Added `DiagnosticsLogEntry` and `DiagnosticsLogFilter` model helpers for read-only filtering and recent-row limiting
  * Recorded app startup, daemon snapshot, and IPC error events in the personal UI without adding any device-changing diagnostics commands
  * Rendered a `RECENT APP / IPC LOG` diagnostics panel and documented that external daemon/journal log tailing remains pending

* Added dashboard-level named personal preset bundles to the native personal UI:
  * Added `PersonalPreset` model support for Go Live, Desktop Focus, Late Night, and FX Panic command bundles spanning routing, lighting, volume/safety, and voice-effect state
  * Wired a `Personal presets` button group into the dashboard Profiles / Scenes panel, prioritizing the safety-style FX Panic action before normal presets
  * Added focused `app_model` coverage for bundle ordering, safety preset selection, descriptions, and typed command membership

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
  * Protected HTTP endpoints require a bearer `Authorization` header
  * `/api/websocket` may also use `?token=<token>` because browser websocket clients cannot reliably set auth headers
  * The bundled browser UI can be unlocked by visiting `/?token=<token>`, which stores a same-origin HttpOnly session cookie and redirects back to `/` for later API and websocket requests
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

### 2026-04-28

* Reviewed and hardened local AI-generated changes:
  * Limited query-string auth for direct API calls to `/api/websocket`; ordinary protected HTTP endpoints require a bearer `Authorization` header or the browser UI auth cookie
  * Added browser UI auth cookie bootstrap: visiting `/?token=<token>` sets an HttpOnly SameSite session cookie, redirects back to `/`, and only accepts that cookie for same-origin browser API/websocket requests
  * Removed `GOXLR_HTTP_TOKEN` from public daemon status/config responses
  * Restores enabled headphone EQ state during device initialization after daemon restart or reconnect
  * Starts EasyEffects with non-blocking `spawn()` during retry instead of waiting for the process to exit
  * Added additional headless/session error detection for EasyEffects auto-load failures
  * Added Headphone EQ commands to the shutdown/sleep/wake disk-write guard
  * Re-applies headphone EQ state after deleting a profile, and resets current EQ to default if the active profile is deleted
  * Launcher now verifies the current user's IPC socket with `Ping` instead of trusting any process named `goxlr-daemon`
  * Manual tray preflight now runs in a blocking background task so the event loop stays responsive
  * ALSA `split.conf` preflight now warns only instead of mutating distro-owned ALSA files
  * Removed Debian `postinst` mutation of distro-owned ALSA `split.conf`
  * Restored deprecated `OpenUi` IPC command as an alias for `Activate` for existing clients
  * Added optional HTTP bearer-token support for CLI `--use-http` via `--http-token` or `GOXLR_HTTP_TOKEN`
  * Added serde defaults for new IPC status fields so newer clients tolerate older daemon status payloads
  * Runs EasyEffects EQ application in a blocking task with a hard process timeout to avoid wedging async device handling
* Started a personal native Rust UI crate:
  * Added `goxlr-personal-ui` workspace member using `eframe`/`egui` and direct `goxlr-ipc` access instead of the browser/Tauri wrapper UI
  * Added a first native control panel with daemon IPC polling, connection/profile/status display, MVP channel sliders, `Safe Now`, ClipGuard, headphone limiter, and headphone EQ toggles
  * Added model tests for MVP channels, `Safe Now` command mapping, and disconnected status text
  * Added first personal scene buttons: `Gaming`, `Music`, `Night`, `Call`, and `Safe Now`, each mapped to practical volume/safety/EQ command bundles
  * Debounced native volume sliders so drag updates coalesce per channel before sending IPC volume commands
  * Added JSON scene config loading for the personal native UI at `$XDG_CONFIG_HOME/goxlr-personal-ui/scenes.json` or `~/.config/goxlr-personal-ui/scenes.json`; missing config files are created with the default scenes so volumes/profile names can be edited without recompiling
  * Added a `Reload scenes` button to re-read the JSON scene config while the native UI is running; failed reloads keep the previous scene buttons and show the parse/load error in the UI
  * Added a first in-app scene editor for renaming scenes, editing per-channel volume actions, setting safety/EQ actions, and saving changes back to the JSON scene config
  * Added in-app scene editor controls to add, delete, and reorder scenes before saving them back to the JSON scene config
  * Added a native UI device picker for multi-GoXLR setups; commands now route to the selected device instead of failing when more than one mixer is reported
  * Improved the scene editor with explicit `Leave unchanged`, `Set on`, and `Set off` controls for optional ClipGuard, headphone limiter, and headphone EQ actions, plus a clearable `Load EQ profile` action
  * Added a compact `Quick actions` view to the personal native UI with fast scene buttons, `Safe Now` priority, quick safety enable buttons, volume sliders, refresh, and a toggle back to the full editor/control view
  * Added a `Mini window` mode for the personal native UI that switches into quick actions, resizes to a smaller always-on-top control window, and can restore the normal full window
  * Added optional Linux StatusNotifier system tray support for the personal native UI behind the `system-tray` Cargo feature, with tray actions for showing full/mini windows, applying `Safe Now`/`Gaming`/`Music` scenes, refreshing daemon status, and quitting
  * Added a legacy `/tmp/goxlr.socket` IPC fallback for the personal native UI so it connects to the currently installed autostart daemon while still preferring the newer runtime-dir socket path
  * Redesigned the personal native UI default view into a GoXLR-style mixer dashboard with charcoal/cyan styling, tab-like navigation, a left profiles/scenes panel, vertical channel strips, a right device/status card, and configuration moved behind the `Configuration` view
  * Added an active playback stream monitor to the personal native UI using `pactl --format=json`, showing currently playing apps, their routed output sink, volume, mute, and paused state in the mixer dashboard
  * Added click-to-route playback buttons in the active playback panel so currently playing app streams can be moved to GoXLR `System`, `Game`, `Music`, `Chat`, or `Sample` outputs via `pactl move-sink-input`
  * Added persistent app auto-routing rules to the personal native UI JSON config; default rules route `Spotify` to GoXLR `Music` and `Discord` to GoXLR `Chat`, and the worker applies matching rules to active playback streams with `pactl move-sink-input`
  * Added an in-app audio routing rule editor so app-match rules can be added, deleted, reordered, enabled/disabled, retargeted to GoXLR `System`/`Game`/`Music`/`Chat`/`Sample`, and saved back to the personal UI config without hand-editing JSON

### 2026-04-29

* Added one-click persistent routing from active playback streams:
  * Active streams now preserve the PulseAudio/PipeWire application name separately from the display label so rules save as app matches like `Firefox` instead of `Firefox — YouTube Music`
  * Added `Always <target>` buttons beside the manual route buttons to save or update an enabled app auto-routing rule and immediately move the current stream to the selected GoXLR output
* Added per-stream mute controls to the personal native UI active playback panel:
  * Each active playback stream now shows `Mute stream` or `Unmute stream` based on its current PipeWire/PulseAudio mute state
  * The worker applies mute changes with `pactl set-sink-input-mute <stream_id> 1|0` and refreshes the active stream snapshot afterward
* Added personal UI polish for safer config editing and daily audio routing:
  * Scene/routing JSON saves now keep a `.bak` copy of the previous config before overwriting it
  * The active apps panel separates one-off `Move now` routing from persistent `Always route` buttons and renames the configuration tab to `Config / Routing`
  * Active playback streams with reported volume now expose an inline volume slider backed by `pactl set-sink-input-volume <stream_id> <percent>%`
  * Header helper buttons can launch `pavucontrol` and `qpwgraph` for deeper PipeWire/PulseAudio routing inspection

* Added the first feature-parity `Mic` page to the personal native UI:
  * The top navigation now opens a dedicated mic processing page alongside the mixer dashboard and config/routing view
  * The mic page exposes mic type selection, mic gain, gate enable/threshold/attenuation, de-esser, compressor threshold/ratio/makeup gain, ClipGuard threshold, and headphone limiter threshold controls
  * Scene JSON can now include mic/safety processing actions such as `mic_type`, `mic_gain`, `gate_*`, `compressor_*`, `deesser`, `clip_guard_threshold`, and `headphone_limiter_threshold`
  * Added command mappings for mic setup, gate, compressor, de-esser, safety thresholds, saving the active mic profile, and reloading settings
* Added the next feature-parity `Effects` page to the personal native UI:
  * The top navigation now opens an `Effects` page for fast GoXLR voice-FX control without launching the browser/web UI
  * Added daily quick presets for `FX Off`, `Clean Reverb`, `Robot Fun`, and `Hard Tune`
  * Added native command mappings for active FX preset selection, FX enable, reverb, echo, pitch, gender, megaphone, robot, and hard tune controls
* Added a practical web-UI parity checklist for the personal native UI:
  * New `personal-ui/PARITY_CHECKLIST.md` tracks implemented areas, remaining parity gaps, priorities, and the TDD implementation rule for future chunks
* Added the first feature-parity `Lighting` page to the personal native UI:
  * The top navigation now opens a `Lighting` page for quick GoXLR colour theme changes without launching the browser/web UI
  * Added daily quick themes for `Dim White`, `Broadcast Red`, `Cool Blue`, and `Lights Off`
  * Added native command mappings for animation mode, global colour, all-fader colours/display style, button-group colours, and simple accent colour
* Expanded the personal native `Effects` page with first-pass detailed controls:
  * Added amount sliders for reverb, echo, pitch, gender, and megaphone
  * Added style button groups for reverb, echo, pitch, gender, megaphone, robot, and hard tune
  * Added model-level tests for the new `EffectsAmountControl` and `EffectsStyleGroup` command coverage
* Made the main personal native UI content scrollable below the fixed header/navigation:
  * Dedicated pages such as Effects, Mic, Lighting, and Config/Routing can now scroll vertically and horizontally when their controls do not fit the window
  * Added model-level coverage for the both-axis scrollable main-content layout policy (`ContentLayoutPolicy`)

* Expanded the personal native `Lighting` page into a first-pass colour editor:
  * Added animation mode/modifier/waterfall controls and per-target colour buttons for simple colours, faders, button groups/buttons, encoders, and sampler select buttons
  * Added native command mappings for per-fader colours/display style, button colours/off styles, encoder colours, sampler colours/off styles, animation modifiers, and waterfall direction
  * Added model-level tests for `LightingSimpleColourTarget`, `LightingFaderColourTarget`, `LightingButtonColourTarget`, `LightingTripleColourTarget`, and `LightingAnimationControl`

### 2026-05-02

* Polished the personal native `Lighting` page layout after screenshot review:
  * Quick themes now render as tighter, adaptive-width cards across the available window instead of a cramped two-column label grid
  * Detailed Lighting editor panels now use smaller model-backed layout widths and wrapped panel rows, reducing wasted empty space and horizontal scrolling at normal window sizes
  * Follow-up pass tightened card gaps/panel widths so an 800px-wide window fits the four quick themes without clipping the `Lights Off` card
  * Screenshot-driven wrap fix now forces Lighting cards/editor panels back to vertical content flow, uses a compact grid for animation mode/modifier/waterfall controls, and gives quick-theme cards a shared minimum height so controls no longer collapse into skinny one-character columns
  * Added `LightingLayoutPolicy` model coverage for the card/panel sizing and animation grid used by the dense Lighting editor
* Polished the personal native `Effects` page layout after screenshot review:
  * Quick presets now render as compact wrapped cards that keep the button, description, and command count together instead of a left-heavy two-column grid
  * Tightened card sizing after manual screenshot QA so the command count stays horizontal, card height is lower, all four presets fit cleanly at the normal ~800px window width, and the preset row stays top-aligned with shared card heights
  * Effects detail panels now use model-backed compact widths and wrapped panel flow so amount/style controls use normal window widths more gracefully
  * Header controls now wrap instead of clipping the external audio tool buttons at narrower window widths
  * Added `EffectsLayoutPolicy` model coverage for quick-preset card sizing and compact detail panel widths
* Added the first native web-UI-style routing matrix to the personal native `Config / Routing` page:
  * Added a matrix of GoXLR input devices (`Mic`, `Chat`, `Music`, `Game`, `Console`, `Line In`, `System`, `Samples`) to output devices (`Headphones`, `Broadcast`, `Chat Mic`, `Sampler`, `Line Out`)
  * Each matrix cell shows daemon-confirmed route state (`Active`, `Off`, or `Unknown`) as a compact centered badge and keeps explicit `On` / `Off` controls backed by typed `PersonalCommand::SetRouter(input, output, enabled)` mapping to `GoXLRCommand::SetRouter`
  * Fixed screenshot-found badge layout regression by constraining routing matrix cells and badges to compact heights, avoiding `centered_and_justified` expansion that stretched active badges into full-height green columns
  * Tightened the verified routing matrix one more pass for commit-readiness: smaller cell slots, smaller state badges, shorter fixed-size `On` / `Off` buttons, and reduced grid gaps so the matrix is less tall and repetitive while keeping deliberate explicit route commands
  * Added model-level `RoutingMatrixModel`, `RoutingMatrixRoute`, `RoutingMatrixLayoutPolicy`, and `RoutingStateBadge` tests for the matrix inputs, outputs, labels, live state lookup, compact non-stretching state-badge styling, cell command generation, dense layout sizing, and backend command mapping
* Continued native web-UI parity from the latest composite screenshot review:
  * Effects `STYLES` now renders style groups as bounded wrapped cards with fixed-size style buttons, preventing labels such as `Natural`, `Medium`, and `Hard` from collapsing into vertical one-letter stacks
  * The Mic page processing panels now use wrapped, model-backed panel widths/gaps so the compressor/safety section can wrap below instead of clipping off the right edge at normal window widths
  * Added named routing preset cards above the routing matrix (`Broadcast Mix`, `Chat Mic`, `Line Out Safe`) backed by explicit `SetRouter(input, output, enabled)` command bundles
  * The Mixer dashboard now uses wrapped layout policies for the scene/device panels and channel-strip row, reducing the mostly-empty fixed horizontal layout and making the page degrade better at narrower sizes
  * Added model-level coverage for `EffectsLayoutPolicy` style-card sizing, `MicLayoutPolicy`, `MixerLayoutPolicy`, and `RoutingPreset` command bundles
* Added a broad screenshot-driven native UI layout containment pass:
  * Introduced reusable bounded panel helpers with fixed widths and natural heights, preventing panels from stretching across huge wide-window regions, collapsing child controls into vertical-letter buttons, or stepping diagonally because of sentinel-height allocation
  * Applied fixed/minimum action-button widths to known screenshot offenders including `Save mic profile`, `Reload settings`, `Enable ClipGuard`, `Enable limiter`, `Enable EQ`, compressor ratio buttons, hard-tune controls, and long Lighting off-style controls
  * Kept Mic, Effects, Lighting, Mixer, and Config/Routing content in left-aligned wrapped dashboard rows with bounded cards/panels instead of full-width horizontal strips, while preserving horizontal scrolling for intentional wide tables such as the routing matrix
  * Extended `ContentLayoutPolicy` model coverage for reusable no-vertical-text button widths, bounded panel behavior, top-aligned wrapped rows, and scrollable main content
* Added a follow-up design polish pass after manual screenshot QA still looked messy:
  * Replaced one-off page headings with consistent section headers on Mic, Effects, Lighting, Mixer, and Config/Routing so each page has a clear title, short context line, and constrained description width
  * Fixed the diagonal/stair-step card look by removing the old fixed-height preallocation inside bounded panel helpers and using top-aligned wrapped rows for card groups
  * Widened the Effects `STYLES` section so style cards form a cleaner compact grid instead of a long sparse vertical column
  * Wrapped Config/Routing scene controls plus volumes/safety controls into bounded panels, keeping the routing matrix readable while making the page less like loose controls on a blank canvas
  * Fixed the egui frame sizing regression found in follow-up screenshots by preallocating bounded frame slots with zero minimum height, so Mic/Effects/Config cards shrink to natural content height instead of stretching to the scroll viewport bottom
  * Completed the three requested screenshot-polish follow-ups: centered the bounded page body in very wide windows with a wider 1320px content policy, made Lighting's detailed editor denser by flowing the encoder/sampler card with the fader/button cards, and tightened the routing matrix cells/badges/buttons/gaps so the explicit On/Off controls are less visually repetitive
* Continued native web-UI feature parity with a first-pass System tab:
  * Added a dedicated `System` view mode and top navigation tab for safe daily device settings
  * Added model-backed System action cards for mute hold duration, VC mute also mutes Chat Mic, monitor-with-FX, lock faders, VOD mode, and reload settings
  * Added typed `PersonalCommand` mappings for `SetMuteHoldDuration`, `SetVCMuteAlsoMuteCM`, `SetMonitorWithFx`, `SetLockFaders`, and `SetVodMode`; intentionally kept destructive profile create/delete operations out of this first-pass daily settings page
* Ported the next ordered native web-UI parity batch across Mic, Effects, Headphone EQ, and Sampler:
  * Added first-pass Mic EQ controls with model-backed mini/full EQ band helpers and typed gain/frequency command mappings, plus guarded Mic profile load/save/save-as/delete actions that require a same-action second click before destructive/profile-switching commands are sent
  * Added first-pass advanced Effects DSP actions for reverb decay, echo feedback, pitch character, megaphone post gain, robot threshold, and hard-tune source/default command paths
  * Added dedicated `Headphone EQ` and `Sampler` view modes/tabs; Headphone EQ exposes enable/disable, preamp, and ten-band gain/frequency/Q command controls, while Sampler exposes bank cards, per-pad play/stop mode, random order, play-next, and stop playback controls
  * Added model-level tests for the ordered parity batch and expanded `PersonalCommand -> GoXLRCommand` mapping coverage; focused `app_model` verification now covers 79 tests
* Extended the Sampler page with the next safe workflow-settings parity chunk:
  * Added model-backed workflow actions for clearing sample process errors, toggling sampler reset-on-clear, and setting a short sampler fade duration
  * Added safe sample trim reset buttons for slot 0 start/stop percentages on each bank/pad while continuing to defer file import/removal workflows
  * Added typed `PersonalCommand` mappings for `ClearSampleProcessError`, `SetSamplerResetOnClear`, `SetSamplerFadeDuration`, `SetSampleStartPercent`, and `SetSampleStopPercent` with focused `app_model` coverage
* Added first-pass Mixer fader assignment and mute-behaviour parity controls:
  * Added a bounded `FADER ASSIGNMENT` panel on the Mixer dashboard with model-backed Fader A-D assignment controls for daily GoXLR channels
  * Added first-pass fader mute-target controls for All, Stream, Voice Chat, and Phones mute behaviours
  * Added typed `PersonalCommand` mappings for `SetFader` and `SetFaderMuteFunction` with focused model-level coverage
* Polished the personal native `Headphone EQ` page after screenshot review:
  * Replaced the sparse wrapped/staggered ten-band card flow with a compact fixed 5x2 band grid
  * Tightened the Headphone EQ panel width and added model-level layout policy coverage so the editor reads as one cohesive equalizer instead of scattered cards in a tall empty panel
* Added first-pass Mixer scribble-strip parity controls:
  * Added a bounded `SCRIBBLE STRIPS` panel on the Mixer dashboard for Fader A-D hardware strip labels, numbers, icon presets, and invert toggles
  * Added typed `PersonalCommand` mappings for `SetScribbleIcon`, `SetScribbleText`, `SetScribbleNumber`, and `SetScribbleInvert` with focused model-level coverage
* Added first-pass Mixer monitor-mix selector parity:
  * Added a bounded `MONITOR MIX` panel on the Mixer dashboard for choosing Headphones, Broadcast, Chat Mic, or Line Out as the hardware monitor source
  * Added typed `PersonalCommand` mapping for `SetMonitorMix(OutputDevice)` with focused model-level coverage
* Added first-pass Mixer submix parity controls:
  * Added a bounded `SUBMIX` panel on the Mixer dashboard for enabling/disabling submix mode, setting conservative channel volume presets, linking/unlinking daily channels, and choosing output mix A/B routing
  * Added model-backed daily submix controls for Mic, Chat, Music, Game, Console, Line In, System, and Sample channels plus Headphones, Broadcast, Chat Mic, and Line Out output mix routing
  * Added typed `PersonalCommand` mappings for `SetSubMixEnabled`, `SetSubMixVolume` (percentage presets converted to the backend raw 0-255 volume scale), `SetSubMixLinked`, and `SetSubMixOutputMix` with focused model-level coverage
* Added guarded first-pass Effects preset management:
  * Added a bounded `EFFECT PRESETS` panel on the Effects page for named-slot load, rename-active, and save-active workflows
  * Added same-action second-click confirmation before dispatching effect preset load/rename/save commands
  * Added typed `PersonalCommand` mappings for `LoadEffectPreset`, `RenameActiveEffectPreset`, and `SaveActiveEffectPreset` with focused model-level coverage
* Added guarded Headphone EQ profile workflows:
  * Added a bounded `EQ PROFILES` panel on the Headphone EQ page for named-slot load, save-as, and delete workflows
  * Added same-action second-click confirmation before dispatching headphone EQ profile load/save/delete commands
  * Added model-backed `HeadphoneEqProfileAction` coverage and layout helpers for the guarded profile panel
* Added guarded first-pass main profile workflows:
  * Added a bounded `MAIN PROFILE` panel on the System page for named-slot full-profile load, save-active, save-as, create, and delete workflows
  * Added same-action second-click confirmation before dispatching full-profile load/save/create/delete commands
  * Added typed `PersonalCommand` mappings for `NewProfile`, `LoadProfile`, `SaveProfile`, `SaveProfileAs`, and `DeleteProfile` with focused model-level coverage
* Added guarded lighting-only profile loading:
  * Added a bounded `LIGHTING PROFILE` panel on the Lighting page for loading only colour settings from the named personal profile slot
  * Added same-action second-click confirmation before dispatching the colour-only profile load command
  * Added typed `PersonalCommand::LoadProfileColours` mapping to `GoXLRCommand::LoadProfileColours` with focused model-level coverage
* Added visual routing rule diff support:
  * Added model-backed `RoutingRuleDiff` / `RoutingRuleDiffStatus` coverage that compares persistent app routing rules with currently active playback stream routes
  * Added a bounded `Routing rule diff` panel on Config / Routing showing current route, desired route, and status (`Matched`, `Move needed`, `Waiting`, `Missing target`, or `Disabled`)
  * Added an `Apply pending moves` action that reuses existing safe routing-move commands for rules that have a live stream and need moving
* Added a follow-up screenshot-driven compactness pass for the latest composite QA:
  * Headphone EQ band cards now use equal-height fixed slots inside the 5x2 grid so the equalizer no longer reads as a diagonal/stair-stepped card layout
  * Sampler bank panels now render the four pads as compact two-by-two slot cards, reducing the tall repeated button wall while keeping file import/removal deferred
  * Mixer fader assignment now combines each fader's channel assignment and mute-behaviour controls into compact Fader A-D cards, reducing the very long single-column hardware section
* Added a read-only Diagnostics / Status page to the personal native UI:
  * The top navigation now exposes daemon connection state, daemon version, selected GoXLR device, profile names, detected-device count, and desktop-audio status without sending device-changing commands
  * The page lists IPC socket candidates, including the legacy `/tmp/goxlr.socket` fallback, and highlights which candidates exist for faster local troubleshooting
  * Added model-level diagnostics layout/status-row coverage and wired the page into `AppViewMode::Diagnostics`
* Expanded the personal native `Effects` page advanced DSP defaults:
  * Broadened the `ADVANCED DSP` panel beyond the original six defaults to include reverb early/tail/pre-delay/colour/mod, echo tempo/delay/cross-feedback, robot gain/frequency/width/waveform/pulse/dry mix, and hard-tune amount/rate/window/source buttons
  * Kept the controls as safe typed quick-default buttons rather than arbitrary sliders, so deeper DSP parity improves without adding a fragile full parameter editor yet
  * Added model-level `EffectsAdvancedControl` coverage for the broader advanced default set
* Added a read-only Mic setup guide panel:
  * The Mic page now includes wizard-style guidance for choosing mic type, setting gain before processing, tuning gate/de-ess, and adding compressor/makeup gain last
  * The panel explicitly calls out that live mic metering is not exposed in the current IPC snapshot, so the guide is status-only and dispatches no device-changing commands
  * Added model-level `MicSetupGuideStep` and `MicLayoutPolicy` coverage for the guidance panel and read-only live-meter placeholder
* Added a read-only `About / Implemented Parity` page:
  * The top navigation now includes an About tab summarizing the native personal UI's implemented and partial parity areas inside the app
  * The summary covers Mixer, Routing, Mic, Effects, Lighting, Headphone EQ, Sampler, Profiles, and Diagnostics so manual QA can quickly see what is intentionally complete versus deferred
  * Added model-level `AboutLayoutPolicy` and `ImplementedParityItem` coverage and wired the page into `AppViewMode::About`
* Added lightweight local-run manual QA notes for the personal native UI:
  * Created `personal-ui/MANUAL_QA.md` with the current running app session/PID, feature/page spot-check checklist, and expected safe behavior per page
  * Recorded that non-interactive COSMIC Wayland screenshot capture via `cosmic-screenshot --interactive=false` failed through the portal, so fresh visual evidence still needs a manual/user screenshot path before marking the checklist item fully complete
* Confirmed the personal native Mic safety preset / threshold workflow parity status:
  * Marked the checklist row complete because existing typed coverage already includes `Safe Now`, dashboard personal safety presets, quick-action safety commands, and app-config scene parsing for gate/compressor/de-ess thresholds plus ClipGuard/headphone-limiter thresholds
  * No new device-mutating controls were added in this cleanup; it only documents the already verified safe typed workflow coverage
* Confirmed Headphone limiter / ClipGuard controls are complete in the native personal UI checklist:
  * The Mic and compact/full views already expose toggles and threshold controls backed by typed ClipGuard and headphone-limiter commands, while the safety presets keep the protective defaults easy to apply
  * This was a checklist/docs cleanup only; no additional device-changing controls were introduced
* Confirmed direct typed routing backend parity status:
  * Marked the `SetRouter` checklist row complete because the native routing matrix, named routing preset cards, dashboard personal presets, and routing helpers already use explicit typed `PersonalCommand::SetRouter(input, output, enabled)` / safe stream-move workflows
  * This was a documentation/status cleanup; no raw router editor or additional device-changing controls were added
* Confirmed guarded effect preset profile-workflow status:
  * Marked the Profiles/Persistence `Effect preset load/save/rename controls` row complete because guarded named-slot actions and discovered `.preset` browser rows are already backed by typed `LoadEffectPreset`, `RenameActivePreset`, and `SaveActivePreset` command mappings
  * Broader arbitrary preset import/file editing remains tracked separately under the Effects section, so this status cleanup does not overstate exhaustive web UI preset management
* Confirmed routing workflow app-config editing status:
  * Marked the Device/System `personal app settings/config editing` row complete for its routing-workflow scope because Config / Routing already exposes persisted scene editing, routing-rule editing, save/reload actions, JSON backup-on-save, active-stream-to-rule saving, and runtime rule refresh
  * Kept the wording scoped to routing workflow config editing rather than claiming a broad native app settings manager

* Added guarded discovered profile/preset browser panels to the personal native UI:
  * Added model-backed `ProfileBrowser`, `ProfileBrowserKind`, row, and action helpers that discover `.goxlr`, `.goxlrMicProfile`, `.preset`, and `.goxlrHeadphoneProfile` files, mark active rows when snapshot state is available, and build guarded per-row command actions
  * Wired browser panels into System/Main profiles, Mic profiles, Effects presets, Lighting colour-only loads, and Headphone EQ profiles, reusing same-action second-click confirmation before dispatching stateful load/save/delete/rename commands
  * Added focused `app_model` coverage for directory discovery, active-row labeling, and main/mic/effects/lighting/headphone-EQ browser command mapping while keeping free-form import/rename/file management deferred
* Added a guarded sampler sample-file workflow panel to the personal native UI:
  * Added model-backed `SamplerFileAction` helpers for typed-path `AddSample`, guarded index-0 `RemoveSampleByIndex`, and unguarded index-0 `PlaySampleByIndex` verification actions
  * Added a Sampler page path entry and per-bank/per-pad file actions that require same-action second-click confirmation before importing or removing sample files
  * Added typed `PersonalCommand` mappings for `AddSample`, `RemoveSampleByIndex`, and `PlaySampleByIndex`, plus focused `app_model` coverage for command mapping and confirmation semantics
  * Kept richer live sample-list editing, arbitrary remove indexes, and a native file picker deferred until they are needed
* Added a simple native sampler sample browser on top of the guarded sampler file workflow:
  * Added model-backed `SamplerSampleBrowser` and `SamplerSampleRow` discovery for supported audio files (`wav`, `mp3`, `flac`, `ogg`, `aiff`, `aif`, `aac`, `m4a`) in a typed browser directory
  * The Sampler page now lists discovered audio files with `Use path` actions that copy a chosen file into the add-path field without sending device-changing commands
  * Added focused `app_model` coverage for extension filtering, alphabetical row ordering, row paths, and browser layout sizing while keeping live slot sample-list/index editing deferred
* Added daemon-backed live sampler slot/sample rows to the personal native Sampler page:
  * Added `SamplerLoadedSample` and `SamplerSlotSnapshot` models that map daemon `Sampler` bank/pad state into stable rows with sample indexes, names, trim percentages, playback mode/order, and playing/recording/empty status labels
  * The Sampler page now shows a `LIVE SAMPLE SLOTS` panel when daemon slot state is available, with per-index `Play #N`, guarded `Remove #N`, and safe start-0%/stop-100% trim reset actions for the shown sample index
  * Added focused `app_model` coverage for daemon sampler mapping, status labels, dynamic per-index play/remove commands, and guarded remove confirmation semantics

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
