# Personal Native UI Web-Parity Checklist

Scope: practical parity for the personal Rust/egui GoXLR UI in `personal-ui`, not a full clone of every bundled web UI detail. Prioritize controls that are useful during daily desktop use and already have typed `GoXLRCommand` support.

Last updated: 2026-05-05
Branch: `personal-native-ui-safety`

## Status legend

- [x] Implemented in the personal native UI and covered by model-level tests.
- [~] Partially implemented; useful today, but not full web UI parity.
- [ ] Not implemented in the personal native UI yet.
- [defer] Available in the broader utility, but low priority for this personal native UI unless it becomes a real workflow need.

## 1. Shell, connection, and app basics

- [x] Native egui shell for local use.
- [x] Direct `goxlr-ipc` command dispatch.
- [x] Legacy `/tmp/goxlr.socket` fallback path.
- [x] Optional system tray build path.
- [x] Device/status visibility: Diagnostics / Status page exposes connection state, daemon version, selected device, profiles, detected device count, desktop-audio status, and IPC socket candidates.
- [~] Dedicated diagnostics/log viewer page: read-only status/socket diagnostics and a recent in-app IPC event log exist; external daemon/journal log tailing remains pending.
- [x] Multi-device picker, if more than one GoXLR is attached.

Priority: keep stable; only extend if debugging or multiple-device use becomes a real need.

## 2. Mixer dashboard and stream controls

- [x] Mixer dashboard page.
- [x] Screenshot-polished wrapped Mixer dashboard layout: scene/device panels and channel-strip row wrap through `MixerLayoutPolicy` instead of leaving a mostly-empty fixed horizontal panel at normal widths; known long actions use fixed/minimum button widths so `Enable ClipGuard`, `Enable limiter`, and `Enable EQ` do not collapse into vertical text.
- [x] Active playback stream monitor.
- [x] Active stream mute controls.
- [x] Persistent route buttons for active streams.
- [x] PipeWire/PulseAudio routing helpers for local Linux workflow.
- [x] Scene-style quick actions for common personal routing/mute states.
- [~] First-pass fader assignment editor: Mixer dashboard exposes compact Fader A-D cards with assignment buttons for daily channels (Mic, Chat, Music, Game, Console, Line In, System, Sample), each fader's daily mute behaviour controls, and direct current-state buttons for Unmute / Mute target / Mute all, backed by typed `SetFader`, `SetFaderMuteFunction`, and `SetFaderMuteState` commands; less-used output/monitor assignments, hold/toggle behaviour, and rarer mute targets remain deferred.
- [~] First-pass fader mute-behaviour editor: Mixer dashboard exposes safe mute-target buttons for All, Stream, Voice Chat, and Phones backed by typed `SetFaderMuteFunction` commands, plus explicit current-state buttons backed by typed `SetFaderMuteState`; hold/toggle and less-used mute targets remain pending.
- [~] First-pass scribble strip editor: Mixer dashboard exposes Fader A-D hardware strip label, number, icon preset, and invert buttons backed by typed scribble commands; free-text entry and full icon browsing remain deferred.
- [x] First-pass monitor mix selector: Mixer dashboard exposes safe hardware monitor-source buttons for Headphones, Broadcast, Chat Mic, and Line Out backed by typed `SetMonitorMix` commands.
- [x] First-pass submix controls: Mixer dashboard exposes safe enable/disable, daily channel volume presets/linking, arbitrary 0–100% channel volume sliders, and output mix A/B routing backed by typed `SetSubMixEnabled`, percent-to-raw `SetSubMixVolume`, `SetSubMixLinked`, and `SetSubMixOutputMix` commands; current daemon-reported channel volume/link state and output mix A/B state are reflected inline when available. Exhaustive output coverage remains deferred.

Priority: low. Daily mute/routing/submix coverage is now strong; remaining Mixer work is mainly true hold/toggle semantics, less-used fader/output edge cases, scribble free-text/icon browsing, and manual-QA/layout polish.

## 3. Routing

- [x] Persistent routing rules UI/config editor.
- [x] Practical route toggles for active desktop streams.
- [x] Uses typed `SetRouter` backend support directly: the routing matrix, named routing preset cards, dashboard personal presets, and persistent-rule move helpers use explicit typed `PersonalCommand::SetRouter(input, output, enabled)` / safe stream-move workflows rather than raw router edits.
- [x] Full matrix-style input-to-output router equivalent to the web UI: native matrix reads daemon route state, shows compact centered state badges with constrained non-stretching cell/badge heights and a denser screenshot-polished row layout, and sends typed `SetRouter(input, output, enabled)` commands for Mic/Chat/Music/Game/Console/Line In/System/Samples to Headphones/Broadcast/Chat Mic/Sampler/Line Out.
- [x] Save/load named routing presets: native preset cards above the matrix apply common explicit `SetRouter` command bundles for Broadcast Mix, Chat Mic, and Line Out Safe.
- [x] Visual diff between desired persistent rules and current active desktop stream routes: Config / Routing now shows current route, desired route, status, and an `Apply pending moves` action for live streams that need moving.

Priority: medium-low. Core routing parity is strong; remaining routing work should be manual-QA/layout polish or dynamic preset/config ergonomics rather than another matrix rewrite.

## 4. Microphone and processing

- [x] Dedicated Mic page.
- [x] Microphone type and gain controls.
- [x] Gate controls.
- [x] De-esser controls.
- [x] Compressor controls.
- [x] ClipGuard and headphone limiter threshold controls.
- [x] Screenshot-polished wrapped Mic layout: processing/safety panels use bounded wrapped panel widths so compressor and safety controls do not clip off the right edge at normal window widths, and screenshot-offender controls such as `Save mic profile`, `Reload settings`, and compressor ratio buttons get fixed/minimum widths to avoid vertical-letter text.
- [x] Practical safety presets / threshold workflows: `Safe Now`, dashboard personal presets, and quick-action safety commands enable ClipGuard, headphone limiter, and headphone EQ; app-config scenes can set gate/compressor/de-ess thresholds plus ClipGuard and headphone limiter thresholds through typed commands.
- [x] Microphone EQ editor: first-pass mini/full EQ frequency and gain command controls.
- [~] Mic setup wizard-style guidance and live level meter: the Mic page now has a read-only setup guide for type/gain/gate/compressor order plus an explicit note that live mic levels are not exposed in the current IPC snapshot; true live meter remains pending.
- [~] Mic profile create/save/load/delete controls: guarded daily load/save/save-as/delete actions are exposed on the Mic page with same-action second-click confirmation, and the page now includes a discovered mic-profile browser for available `.goxlrMicProfile` rows; free-form import/rename workflows remain deferred.

Priority: medium-low after current work. Mic basics are covered; EQ/profile polish can wait unless voice tuning becomes the next focus.

## 5. Voice effects

- [x] Dedicated Effects page.
- [x] Quick FX presets: FX Off, Clean Reverb, Robot Fun, Hard Tune.
- [x] Quick preset layout polish: compact wrapped cards keep preset name, description, and command count together, keep command labels horizontal, share a top-aligned row height, and fit all four quick presets cleanly at normal window width.
- [x] Command mappings for active preset, FX enable, reverb, echo, pitch, gender, megaphone, robot, and hard tune basics.
- [x] Quick buttons for FX on/off, robot, and hard tune.
- [~] Full active preset management: guarded named-slot load, rename-active, and save-active controls exist with same-action second-click confirmation, plus a discovered `.preset` browser for available bundled/user preset files; arbitrary preset import/file editing remains pending.
- [x] Reverb detailed controls: amount/style are native, plus arbitrary clamped sliders for decay, early/tail levels, pre-delay, low/high colour, high factor, diffuse, and modulation speed/depth backed by typed `SetReverb*` commands.
- [x] Echo detailed controls: amount/style are native, plus arbitrary clamped sliders for feedback, tempo, left/right delay, left/right feedback, and cross-feedback backed by typed `SetEcho*` commands.
- [x] Pitch detailed controls: amount/style are native, plus arbitrary clamped pitch character slider backed by typed `SetPitchCharacter` commands.
- [x] Gender detailed controls: amount and style.
- [~] Megaphone detailed controls: enable/style/amount are native, plus arbitrary clamped post-gain slider backed by typed `SetMegaphonePostGain` commands; deeper hidden profile parameters remain pending.
- [x] Robot detailed controls: enable/style are native, plus arbitrary clamped sliders for low/mid/high gain, frequency, and width bands, waveform, pulse width, threshold, and dry mix backed by typed `SetRobot*` commands.
- [x] Hard tune detailed controls: enable/style are native, plus arbitrary clamped sliders for amount, rate, and window backed by typed `SetHardTune*` commands; source remains an explicit advanced default button.

Priority: mostly implemented for daily personal use. Effects now has presets, amount/style controls, broad advanced defaults, and arbitrary sliders for the exposed typed DSP families; future work should focus only on guarded preset/file ergonomics or hidden Megaphone parameters if they become a real need.

Implemented Effects detail chunk:

- Added model-level `EffectsAmountControl` coverage for reverb, echo, pitch, gender, and megaphone amount sliders.
- Added model-level `EffectsStyleGroup` coverage for reverb, echo, pitch, gender, megaphone, robot, and hard tune style buttons.
- Expanded `EffectsAdvancedControl` coverage and the Effects `ADVANCED DSP` panel from a handful of defaults to broader quick-default buttons for reverb early/tail/pre-delay/colour/mod, echo tempo/delay/cross-feedback, robot gain/frequency/width/waveform/pulse/dry mix, and hard-tune amount/rate/window/source.
- Added arbitrary clamped `EffectsReverbSlider` coverage and a `REVERB SLIDERS` section for decay, early/tail levels, pre-delay, low/high colour, high factor, diffuse, and modulation speed/depth.
- Added arbitrary clamped `EffectsEchoSlider` coverage and an `ECHO SLIDERS` section for feedback, tempo, left/right delay, left/right feedback, and cross-feedback.
- Added arbitrary clamped `EffectsPitchSlider` coverage and a `PITCH SLIDERS` section for pitch character.
- Added arbitrary clamped `EffectsMegaphoneSlider` coverage and a `MEGAPHONE SLIDERS` section for post gain.
- Added arbitrary clamped `EffectsRobotSlider` coverage and a `ROBOT SLIDERS` section for low/mid/high gain, frequency, and width bands, waveform, pulse width, threshold, and dry mix.
- Added arbitrary clamped `EffectsHardTuneSlider` coverage and a `HARD TUNE SLIDERS` section for amount, rate, and window.
- Added `EffectsLayoutPolicy` coverage for wrapped quick-preset cards, compact detail panel widths, and bounded style-group cards/buttons so the `STYLES` panel avoids vertical-letter button wrapping.
- Wired the Effects page to render amount sliders, style button groups, Reverb/Echo/Pitch/Megaphone/Robot/Hard Tune arbitrary DSP sliders, and advanced DSP defaults backed by typed `PersonalCommand` mappings.

## 6. Lighting

- [x] Dedicated Lighting page.
- [x] Global colour preset buttons via quick themes.
- [x] Fader colours and display style controls: first-pass editor covers all faders plus fader A-D colour pairs and display style buttons.
- [x] Button colours and off-style controls: editor covers button groups plus individual Cough/Bleep, effect preset/type, FX, and sampler pad/clear buttons backed by typed `SetButtonColours` / `SetButtonOffStyle` commands.
- [x] Button group colour controls: editor covers fader mute, effect selector, and effect types groups.
- [x] Simple colour targets: editor covers Global, Accent, and Scribble 1-4.
- [x] Encoder colours: editor covers Reverb, Pitch, Echo, and Gender encoder colour triplets.
- [x] Sampler select colours and off styles: editor covers Sampler Select A/B/C colour triplets and off-style control.
- [x] Animation mode controls: editor covers Simple, Rainbow, Ripple, Retro, and None.
- [x] Animation modifiers and waterfall direction.
- [x] Layout polish for dense Lighting controls: adaptive quick-theme cards and wrapped editor panel rows reduce the cramped left column, fit the four quick themes at an 800px-wide window, keep card heights consistent, keep animation/editor controls in vertical panel flow instead of skinny one-character wrapped columns, and give long fader/button/sampler style actions fixed widths.
- [~] Load only lighting from profile: guarded named-slot colour-only load exists on the Lighting page with same-action second-click confirmation, and the page now includes a discovered `.goxlr` browser for guarded per-row lighting-only loads; arbitrary import/rename/location management remains pending.

Priority: mostly implemented for daily personal use. Remaining Lighting work is profile/file-location management and targeted UX polish if the current editor feels too broad or click-heavy.

Implemented first Lighting chunk:

- Added `AppViewMode::Lighting`.
- Added quick themes: `Dim White`, `Broadcast Red`, `Cool Blue`, and `Lights Off`.
- Mapped themes to `SetAnimationMode`, `SetGlobalColour`, `SetAllFaderColours`, `SetAllFaderDisplayStyle`, `SetButtonGroupColours`, and `SetSimpleColour`.
- Added model-level tests for navigation, theme command bundles, and `PersonalCommand -> GoXLRCommand` mappings.

Implemented Lighting colour-editor chunk:

- Added `LightingSimpleColourTarget`, `LightingFaderColourTarget`, `LightingButtonColourTarget`, `LightingTripleColourTarget`, and `LightingAnimationControl` models.
- Added editor panels for animation, simple colours, faders, buttons, encoders, and sampler select colours.
- Added typed command mappings for per-fader colours/display styles, button colours/off styles, button-group off styles, encoder colours, sampler colours/off styles, animation modifiers, and waterfall direction.

## 7. Sampler

- [x] Dedicated Sampler page.
- [x] Compact two-by-two Sampler bank card layout: each bank now groups TopLeft/TopRight/BottomLeft/BottomRight into equal slot cards instead of one long repeated vertical button wall.
- [x] Active sampler bank selector.
- [x] Play next sample for each pad.
- [x] Stop sample playback.
- [x] Playback mode/order controls: first-pass play/stop mode and random order actions.
- [~] Add/remove sample controls: guarded typed-path import and remove actions are exposed per bank/pad with same-action second-click confirmation, plus a simple directory sample browser and daemon-backed live slot/sample list with per-index play/remove controls; deeper waveform/drag/drop editing remains pending.
- [x] Sample start/stop percentage controls: safe bounded start presets (0%, 25%, 50%) and stop presets (50%, 75%, 100%) are exposed per live sample index when daemon slot state is available, and live sample rows now include arbitrary 0–100% Start/Stop sliders backed by typed `SetSampleStartPercent` / `SetSampleStopPercent` commands.
- [x] Clear sample process error.
- [x] Sampler reset-on-clear setting.
- [x] Sampler fade duration setting.

Priority: mostly implemented for safe personal workflows. The native page covers playback/bank controls, workflow settings, guarded typed-path add/remove actions, a simple supported-audio directory browser, daemon-backed live sample slot/index rows, and per-index custom Start/Stop trim sliders. Remaining work is richer waveform editing, drag/drop, and bulk sample management.

## 8. Profiles and persistence

- [~] Main profile create/load/save-as/delete controls: guarded named-slot full-profile load, save-active, save-as, create, and delete actions exist on the System page with same-action second-click confirmation, plus a discovered `.goxlr` browser with guarded per-row load, lighting-only load, save-as, and delete actions; arbitrary import/rename/location management remains pending.
- [~] Mic profile create/load/save-as/delete controls: guarded Mic-page actions exist for a named profile slot with same-action second-click confirmation, plus a discovered mic-profile browser for available profile rows; free-form import/rename workflows remain pending.
- [x] Effect preset load/save/rename controls: guarded named-slot actions exist on the Effects page with same-action second-click confirmation, plus a discovered preset browser for available `.preset` rows; broader arbitrary preset import/file editing remains tracked separately under Effects.
- [~] Headphone EQ profile save/load/delete controls: guarded named-slot load, save-as, and delete actions exist on the Headphone EQ page with same-action second-click confirmation, plus discovered `.goxlrHeadphoneProfile` browser rows for available headphone EQ profiles; arbitrary import/rename/location management remains pending.
- [x] Named personal presets for common routing, lighting, and effect states: dashboard `Personal presets` buttons expose Go Live, Desktop Focus, Late Night, and FX Panic bundles backed by explicit typed command lists.
- [x] Clear warning boundaries around destructive profile operations for first-pass mic profile actions.

Priority: medium. Useful once daily control pages are stable, but should be implemented carefully because profile operations are stateful/destructive.

## 9. Device/system settings

- [x] Headphone limiter / ClipGuard controls: Mic and compact/full views expose toggles and thresholds backed by typed `SetClipGuardEnabled`, `SetClipGuardThreshold`, `SetHeadphoneLimiterEnabled`, and `SetHeadphoneLimiterThreshold` commands; safety presets also enable the protective defaults.
- [x] Routing workflow app-config editing: Config / Routing exposes persisted scene editing, routing-rule editing, save/reload actions, JSON backup-on-save, active-stream-to-rule saving, and runtime rule refresh for the personal routing workflows; this row is scoped to routing config rather than a broad app settings manager.
- [x] Mute hold duration.
- [x] Cough button behaviour and mute-target controls: System page exposes hold/toggle mode plus daily mute targets (All, Stream, Voice Chat, Phones) backed by typed `SetCoughIsHold` and `SetCoughMuteFunction` commands.
- [x] VC mute also mutes chat mic.
- [x] Monitor-with-FX toggle.
- [x] Lock faders toggle.
- [x] VOD mode setting.
- [x] Headphone EQ full editor: dedicated tab with enabled/preamp and ten-band gain/frequency/Q command controls; screenshot-polished into a compact fixed 5x2 equal-height band grid instead of a sparse staggered card flow.
- [x] Hardware-first headphone listening presets: Headphone EQ page exposes `Neutral Base`, `Music Detail`, `Game Imaging`, and `Night Safe` command bundles that set headphone monitor source, safe volume, limiter enable/threshold, EQ enable/preamp, and ten-band EQ gain/frequency/Q values for practical listening goals beyond original web-app parity.
- [x] Headphone tuning workflow guidance: Headphone EQ page documents the intended order of route intentionally, gain-stage/preamp first, enable limiter, tune by purpose, and save after real listening.
- [x] General device/system settings page: first-pass safe daily controls for mute hold duration, cough hold/toggle and target, VC/chat mic coupling, monitor-with-FX, fader lock, VOD mode, and reload settings; the System page also shows a read-only live daemon settings snapshot for the same values. Destructive profile operations remain guarded separately.

Priority: medium-low. Add only settings that solve a current annoyance; avoid building a settings junk drawer too early.

## 10. Safety, tests, and maintainability

- [x] Model-level tests for implemented personal UI command bundles.
- [x] Model-level tests for implemented `PersonalCommand -> GoXLRCommand` mappings.
- [x] Reusable screenshot-driven layout containment policy: `ContentLayoutPolicy` covers scrollable main content, natural-height bounded panels, top-aligned wrapped rows, and minimum/wide action button widths used to prevent vertical-letter controls across Mic, Effects, Lighting, Mixer, and Config/Routing.
- [x] README `Local Change Log` updated for functional personal UI changes.
- [x] Known verification commands:
  - `cargo fmt --check`
  - `cargo check -p goxlr-personal-ui`
  - `cargo test -p goxlr-personal-ui --lib --bins --tests`
  - `cargo check -p goxlr-personal-ui --features system-tray`
- [x] Local user install/launcher helper: `personal-ui/scripts/install-local.sh` builds the release personal UI with the `system-tray` feature, installs `~/.local/bin/goxlr-personal-ui`, and writes `~/.local/share/applications/goxlr-personal-ui.desktop` for desktop-menu launching.
- [~] Lightweight screenshot/manual QA notes after local run: `personal-ui/MANUAL_QA.md` now records the current running app session, pages/features to spot-check, and the COSMIC Wayland screenshot-portal failure; fresh page-specific screenshot findings are still pending.
- [x] A small in-app "About / implemented parity" screen: read-only About tab summarizes implemented and partial parity areas so manual QA can distinguish completed daily controls from intentionally deferred full managers/editors.

## Recommended next choices

1. Finish production-readiness review, then commit the current combined safe chunk.
   - Canonical Rust checks and the full personal UI test suite have passed after the dashboard label fix.
   - `MANUAL_QA.md` now records the current running app session, production-use posture, clippy/audit tool blockers, and the remaining manual screenshot gate.
   - Do one final hands-on pass through the running app before committing, but avoid stacking more feature work into this already-large verified slice.

2. If continuing polish before commit, keep it strictly screenshot/manual-QA driven.
   - Effects, Lighting, Mixer, Sampler, Routing, Headphone EQ, System, Diagnostics, and About already have strong daily controls; only adjust layouts or labels that are visibly awkward in the current run.

3. If continuing headphone audio work after commit, prefer measured/listening-driven iteration.
   - Good candidates: refine the four preset EQ curves after actual listening, add a user-editable “My headphones” slot, or add profile-save reminders tied to the current preset.

4. Keep file/workflow-heavy gaps deferred unless they become real workflows.
   - Remaining higher-risk work is waveform/drag-drop/bulk sample editing, daemon/journal log tailing, hidden Megaphone profile parameters, and free-form profile import/rename/location management.

Implemented routing matrix chunk:

- Added a native web-UI-style input/output matrix on the Config / Routing page.
- Added model-level `RoutingMatrixModel` coverage for Mic/Chat/Music/Game/Console/Line In/System/Samples inputs and Headphones/Broadcast/Chat Mic/Sampler/Line Out outputs.
- Added live route-state indicators from the daemon snapshot so each matrix cell shows `Active`, `Off`, or `Unknown` as a compact centered badge while retaining explicit `On`/`Off` commands.
- Tightened the compact badge layout after screenshot QA: cells now reserve a short fixed-height slot, badges use a bounded width/height, and the UI avoids `centered_and_justified` expansion that made active badges fill entire columns.
- Added a final density pass after the fixed screenshot showed the matrix was still too tall/repetitive: `RoutingMatrixLayoutPolicy` now uses smaller cell slots, shorter badges, fixed-size compact `On` / `Off` buttons, and reduced row/column gaps while preserving explicit route commands.
- Added typed `PersonalCommand::SetRouter(input, output, enabled)` mapping to backend `GoXLRCommand::SetRouter` and UI `On`/`Off` buttons per matrix cell.
- Added `RoutingStateBadge` and `RoutingMatrixLayoutPolicy` model coverage so screenshot-driven state-label polish stays testable without brittle widget snapshots.
- Added named routing preset cards above the matrix for `Broadcast Mix`, `Chat Mic`, and `Line Out Safe`; each preset is a deliberate bundle of explicit `SetRouter(input, output, enabled)` commands instead of an optimistic single-toggle state.
- Added a visual routing-rule diff panel that compares saved app-match routing rules against current active playback stream destinations, labels each rule as `Matched`, `Move needed`, `Waiting`, `Missing target`, or `Disabled`, and can apply only the pending live-stream moves.

Implemented screenshot-driven compactness pass after the latest composite QA:

- Headphone EQ band cards now use equal-height fixed slots in the 5x2 grid, eliminating the remaining diagonal/stair-step equalizer layout.
- Sampler bank panels now render TopLeft/TopRight/BottomLeft/BottomRight as compact two-by-two slot cards, reducing the tall repeated button wall while keeping file import/removal deferred.
- Mixer fader assignment now combines each fader's channel assignment and mute-behaviour controls into compact Fader A-D cards, reducing the very long single-column hardware section.

Implemented composite screenshot polish pass:

- Effects `STYLES` now uses bounded wrapped style-group cards and fixed-size style buttons to prevent hard-tune style labels from collapsing into vertical one-letter columns.
- Mic processing panels now use `MicLayoutPolicy` wrapped widths/gaps so compressor and safety controls can move to the next row instead of clipping horizontally.
- Mixer dashboard panels and channel strips now use `MixerLayoutPolicy` wrapping so the page is less dependent on one wide mostly-empty horizontal area.
- Added model-level coverage for the new layout policies, routing preset command bundles, fader assignment/mute behaviour controls, and first-pass scribble-strip command mappings.

Implemented broad layout containment pass:

- Added reusable bounded panel helpers around egui frames so page panels/cards keep fixed widths but natural heights; this keeps wide windows from stretching normal controls into giant horizontal strips, avoids diagonal/stair-step wrapped cards from sentinel-height allocations, and preserves scrolling for intentionally wide matrix/table content.
- Fixed the latest screenshot's vertical-letter offenders with fixed/minimum button widths: Mic `Save mic profile`, `Reload settings`, compressor ratio buttons; Mixer `Enable ClipGuard`, `Enable limiter`, `Enable EQ`; Effects `Hard Tune On`; and long Lighting style/off-style actions.
- Extended `ContentLayoutPolicy` model coverage for bounded panel behavior, top-aligned wrapped rows, scrollable main content, and minimum/wide action button sizing.

Implemented follow-up design polish pass:

- Added consistent section headers to Mic, Effects, Lighting, Mixer, and Config/Routing so the pages have a clearer hierarchy and constrained intro copy instead of floating controls.
- Widened Effects `STYLES` and switched nested style groups to the reusable top-aligned wrapped row helper so style cards form a compact grid.
- Wrapped Config/Routing profile/scene controls and the lower Volumes/Safety controls into bounded panels; the routing matrix remains explicit and readable.
- Fixed the egui bounded-frame sizing regression found in follow-up screenshots: cards/panels now allocate a bounded width slot with zero minimum height before drawing the frame, so they keep natural content height instead of stretching to the scroll viewport bottom.
- Completed the requested wide-window / Lighting density / routing readability pass: the scroll body now centers bounded content in very wide windows, Lighting flows the encoder/sampler card with the fader/button editor cards using a denser 360px panel policy, and the routing matrix uses smaller cells, state badges, buttons, and grid gaps to reduce repeated On/Off visual noise.

Implemented first System settings chunk:

- Added `AppViewMode::System` and a dedicated System tab for safe daily device settings.
- Added model-backed `SystemSettingsAction` controls for mute hold duration, VC mute/chat mic coupling, monitor-with-FX, lock faders, VOD mode, and reload settings.
- Added `PersonalCommand` mappings for `SetMuteHoldDuration`, `SetVCMuteAlsoMuteCM`, `SetMonitorWithFx`, `SetLockFaders`, and `SetVodMode`; destructive profile create/delete workflows remain intentionally omitted from this first-pass daily settings page.
- Added Mixer monitor-mix selector controls backed by `SetMonitorMix(OutputDevice)` for Headphones, Broadcast, Chat Mic, and Line Out.


Implemented ordered parity batch:

- Added Mic EQ models and Mic-page controls for mini/full EQ band gain/frequency command helpers.
- Added guarded Mic profile actions for load, save current, save-as, and delete against an explicit named slot, with profile-switching/destructive actions requiring a same-action second click before dispatch.
- Added first-pass advanced Effects DSP actions for reverb decay, echo feedback, pitch character, megaphone post gain, robot threshold, and hard-tune source.
- Added guarded Effects preset actions for load, rename-active, and save-active against an explicit named slot, with profile-switching/stateful actions requiring a same-action second click before dispatch.
- Added guarded Headphone EQ profile actions for load, save-as, and delete against an explicit named slot, with profile-changing/destructive actions requiring a same-action second click before dispatch.
- Added a dedicated Headphone EQ tab with enable/disable, preamp, and ten-band gain/frequency/Q command controls; follow-up screenshot QA tightened it into a compact fixed 5x2 grid so the bands read as one equalizer surface rather than a tall scattered card layout.
- Added a dedicated Sampler tab with bank cards, pad actions, active-bank selection, play/stop mode, random order, play-next, and stop playback controls.
- Added model-level tests for all five requested chunks plus typed `PersonalCommand -> GoXLRCommand` mappings.

Implemented diagnostics/status chunk:

- Added `AppViewMode::Diagnostics` and a dedicated read-only Diagnostics / Status tab.
- Added model-backed diagnostics rows for connection state, daemon version, selected device, detected device count, main/mic/headphone EQ profiles, and desktop-audio status.
- Added IPC socket candidate visibility, including legacy `/tmp/goxlr.socket`, so local daemon/socket problems can be inspected without leaving the native UI.
- Added `DiagnosticsLayoutPolicy`, `DiagnosticsStatusRow`, and `DiagnosticsStatusSeverity` model coverage; this is status-only and does not send device-changing commands.
- Added a read-only recent app/IPC event log panel backed by `DiagnosticsLogEntry` filtering/row-limit model coverage; external daemon/journal log tailing remains pending.

## Implementation rule for each chunk

For each new parity chunk:

1. Inspect current backend commands and type enums.
2. Add failing focused tests in `personal-ui/tests/app_model.rs`.
3. Implement minimal model and UI wiring in `personal-ui/src/lib.rs`.
4. Update this checklist and README `Local Change Log`.
5. Run focused tests, then full package checks.
6. Commit only after verification passes and the user chooses to commit.
