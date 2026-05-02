# Personal Native UI Web-Parity Checklist

Scope: practical parity for the personal Rust/egui GoXLR UI in `personal-ui`, not a full clone of every bundled web UI detail. Prioritize controls that are useful during daily desktop use and already have typed `GoXLRCommand` support.

Last updated: 2026-05-02
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
- [~] Device/status visibility: enough for basic connection feedback, but not a full device-management screen.
- [ ] Dedicated diagnostics/log viewer page.
- [ ] Multi-device picker, if more than one GoXLR is attached.

Priority: keep stable; only extend if debugging or multiple-device use becomes a real need.

## 2. Mixer dashboard and stream controls

- [x] Mixer dashboard page.
- [x] Active playback stream monitor.
- [x] Active stream mute controls.
- [x] Persistent route buttons for active streams.
- [x] PipeWire/PulseAudio routing helpers for local Linux workflow.
- [~] Scene-style quick actions for common personal routing/mute states.
- [ ] Full fader assignment editor.
- [ ] Full fader mute-behaviour editor: mute target, hold/toggle behaviour, and related per-fader settings.
- [ ] Scribble strip editor: icon, label text, number text, invert.
- [ ] Submix controls: enable, per-channel submix volume/linking, output mix routing.
- [ ] Monitor mix selector.

Priority: medium. Daily mute/routing is already useful; fader assignment and scribbles would improve web parity but are less urgent than Lighting or full Effects.

## 3. Routing

- [x] Persistent routing rules UI/config editor.
- [x] Practical route toggles for active desktop streams.
- [~] Uses typed `SetRouter` backend support indirectly through personal routing workflows.
- [ ] Full matrix-style input-to-output router equivalent to the web UI.
- [ ] Save/load named routing presets.
- [ ] Visual diff between desired persistent rules and current GoXLR route state.

Priority: medium. A full routing matrix is valuable, but the current personal routing buttons cover the main Linux desktop use case.

## 4. Microphone and processing

- [x] Dedicated Mic page.
- [x] Microphone type and gain controls.
- [x] Gate controls.
- [x] De-esser controls.
- [x] Compressor controls.
- [x] ClipGuard and headphone limiter threshold controls.
- [~] Practical safety presets / threshold workflows.
- [ ] Microphone EQ editor: mini/full EQ frequency and gain controls.
- [ ] Mic setup wizard-style guidance and live level meter.
- [ ] Mic profile create/save/load/delete controls.

Priority: medium-low after current work. Mic basics are covered; EQ/profile polish can wait unless voice tuning becomes the next focus.

## 5. Voice effects

- [x] Dedicated Effects page.
- [x] Quick FX presets: FX Off, Clean Reverb, Robot Fun, Hard Tune.
- [x] Command mappings for active preset, FX enable, reverb, echo, pitch, gender, megaphone, robot, and hard tune basics.
- [~] Quick buttons for FX on/off, robot, and hard tune.
- [ ] Full active preset management: load, rename, save.
- [ ] Reverb detailed controls: amount, decay, early/tail level, pre-delay, low/high colour, high factor, diffuse, mod speed/depth.
- [ ] Echo detailed controls: amount, feedback, tempo, left/right delay, left/right feedback, cross-feedback.
- [ ] Pitch detailed controls: amount and character.
- [ ] Gender detailed controls.
- [ ] Megaphone detailed controls: enable, style, amount, post gain.
- [ ] Robot detailed controls: enable, style, gain/frequency/width ranges, waveform, pulse width, threshold, dry mix.
- [ ] Hard tune detailed controls: enable, style, amount, rate, window, source.

Priority: high if the next chunk is "deepen an existing page". This is the best follow-up if you want fewer new tabs and more complete controls.

## 6. Lighting

- [x] Dedicated Lighting page.
- [x] Global colour preset buttons via quick themes.
- [~] Fader colours and display style controls: quick themes set all-fader colours and the lights-off display style, but there is no per-fader editor yet.
- [ ] Button colours and off-style controls.
- [~] Button group colour controls: quick themes cover fader mute, effect selector, and effect types groups.
- [~] Simple colour targets: quick themes and quick actions cover Accent; no full simple-target editor yet.
- [ ] Encoder colours.
- [ ] Sampler pad colours and off styles.
- [~] Animation mode controls: quick themes and buttons cover Simple/None, but not every animation mode.
- [ ] Animation modifiers and waterfall direction.
- [ ] Load only lighting from profile.

Priority: partially implemented. The first preset-based Lighting page is useful now; next Lighting work should add a real colour editor only if quick themes feel too limiting.

Implemented first Lighting chunk:

- Added `AppViewMode::Lighting`.
- Added quick themes: `Dim White`, `Broadcast Red`, `Cool Blue`, and `Lights Off`.
- Mapped themes to `SetAnimationMode`, `SetGlobalColour`, `SetAllFaderColours`, `SetAllFaderDisplayStyle`, `SetButtonGroupColours`, and `SetSimpleColour`.
- Added model-level tests for navigation, theme command bundles, and `PersonalCommand -> GoXLRCommand` mappings.

## 7. Sampler

- [ ] Dedicated Sampler page.
- [ ] Active sampler bank selector.
- [ ] Play next sample for each pad.
- [ ] Stop sample playback.
- [ ] Playback mode/order controls.
- [ ] Add/remove sample controls.
- [ ] Sample start/stop percentage controls.
- [ ] Clear sample process error.
- [ ] Sampler reset-on-clear setting.
- [ ] Sampler fade duration setting.

Priority: defer unless you actively use samples. It has many file/workflow edge cases and less daily value than Lighting or Effects.

## 8. Profiles and persistence

- [ ] Main profile create/load/save-as/delete controls.
- [ ] Mic profile create/load/save-as/delete controls.
- [ ] Effect preset load/save/rename controls.
- [ ] Headphone EQ profile save/load/delete controls.
- [ ] Named personal presets for common routing, lighting, and effect states.
- [ ] Clear warning boundaries around destructive profile operations.

Priority: medium. Useful once daily control pages are stable, but should be implemented carefully because profile operations are stateful/destructive.

## 9. Device/system settings

- [~] Headphone limiter / ClipGuard controls are exposed through the Mic page.
- [~] Some personal app settings/config editing exists for routing workflows.
- [ ] Mute hold duration.
- [ ] VC mute also mutes chat mic.
- [ ] Monitor-with-FX toggle.
- [ ] Lock faders toggle.
- [ ] VOD mode setting.
- [ ] Headphone EQ full editor: enabled, preamp, band gain/frequency/Q.
- [ ] General device/system settings page.

Priority: medium-low. Add only settings that solve a current annoyance; avoid building a settings junk drawer too early.

## 10. Safety, tests, and maintainability

- [x] Model-level tests for implemented personal UI command bundles.
- [x] Model-level tests for implemented `PersonalCommand -> GoXLRCommand` mappings.
- [x] README `Local Change Log` updated for functional personal UI changes.
- [x] Known verification commands:
  - `cargo fmt --check`
  - `cargo check -p goxlr-personal-ui`
  - `cargo test -p goxlr-personal-ui --lib --bins --tests`
  - `cargo check -p goxlr-personal-ui --features system-tray`
- [ ] Lightweight screenshot/manual QA notes for each new page after local run.
- [ ] A small in-app "About / implemented parity" screen, if the checklist becomes useful in the UI itself.

## Recommended next choices

1. Expand Effects into full detailed controls.
   - Best if you want to complete the current Effects page before adding more navigation.
   - More controls, but less new page structure.

2. Expand Lighting into a full colour editor.
   - Best if the quick themes are useful but too limiting.
   - Add per-target colour controls for faders, buttons, encoders, and sampler pads.

3. Add the full routing matrix.
   - Good for web parity, but current Linux stream routing already solves much of the personal workflow.

4. Add Profiles/System page.
   - Useful later, but stateful/destructive enough that it should come after daily controls are comfortable.

## Implementation rule for each chunk

For each new parity chunk:

1. Inspect current backend commands and type enums.
2. Add failing focused tests in `personal-ui/tests/app_model.rs`.
3. Implement minimal model and UI wiring in `personal-ui/src/lib.rs`.
4. Update this checklist and README `Local Change Log`.
5. Run focused tests, then full package checks.
6. Commit only after verification passes and the user chooses to commit.
