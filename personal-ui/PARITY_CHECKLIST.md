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
- [x] Screenshot-polished wrapped Mixer dashboard layout: scene/device panels and channel-strip row wrap through `MixerLayoutPolicy` instead of leaving a mostly-empty fixed horizontal panel at normal widths; known long actions use fixed/minimum button widths so `Enable ClipGuard`, `Enable limiter`, and `Enable EQ` do not collapse into vertical text.
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
- [x] Full matrix-style input-to-output router equivalent to the web UI: native matrix reads daemon route state, shows compact centered state badges with constrained non-stretching cell/badge heights and a denser screenshot-polished row layout, and sends typed `SetRouter(input, output, enabled)` commands for Mic/Chat/Music/Game/Console/Line In/System/Samples to Headphones/Broadcast/Chat Mic/Sampler/Line Out.
- [x] Save/load named routing presets: native preset cards above the matrix apply common explicit `SetRouter` command bundles for Broadcast Mix, Chat Mic, and Line Out Safe.
- [ ] Visual diff between desired persistent rules and current GoXLR route state.

Priority: medium. A full routing matrix is valuable, but the current personal routing buttons cover the main Linux desktop use case.

## 4. Microphone and processing

- [x] Dedicated Mic page.
- [x] Microphone type and gain controls.
- [x] Gate controls.
- [x] De-esser controls.
- [x] Compressor controls.
- [x] ClipGuard and headphone limiter threshold controls.
- [x] Screenshot-polished wrapped Mic layout: processing/safety panels use bounded wrapped panel widths so compressor and safety controls do not clip off the right edge at normal window widths, and screenshot-offender controls such as `Save mic profile`, `Reload settings`, and compressor ratio buttons get fixed/minimum widths to avoid vertical-letter text.
- [~] Practical safety presets / threshold workflows.
- [x] Microphone EQ editor: first-pass mini/full EQ frequency and gain command controls.
- [ ] Mic setup wizard-style guidance and live level meter.
- [~] Mic profile create/save/load/delete controls: guarded daily load/save/save-as/delete actions are exposed on the Mic page with same-action second-click confirmation, but no full browser-style profile manager yet.

Priority: medium-low after current work. Mic basics are covered; EQ/profile polish can wait unless voice tuning becomes the next focus.

## 5. Voice effects

- [x] Dedicated Effects page.
- [x] Quick FX presets: FX Off, Clean Reverb, Robot Fun, Hard Tune.
- [x] Quick preset layout polish: compact wrapped cards keep preset name, description, and command count together, keep command labels horizontal, share a top-aligned row height, and fit all four quick presets cleanly at normal window width.
- [x] Command mappings for active preset, FX enable, reverb, echo, pitch, gender, megaphone, robot, and hard tune basics.
- [x] Quick buttons for FX on/off, robot, and hard tune.
- [~] Full active preset management: load, rename, save still pending.
- [~] Reverb detailed controls: amount/style are native, plus first-pass advanced decay default; early/tail/pre-delay/colour/mod remain pending.
- [~] Echo detailed controls: amount/style are native, plus first-pass feedback default; tempo/delay/cross-feedback remain pending.
- [~] Pitch detailed controls: amount/style are native, plus first-pass character default.
- [x] Gender detailed controls: amount and style.
- [~] Megaphone detailed controls: enable/style/amount are native, plus first-pass post-gain default; deeper parameters remain pending.
- [~] Robot detailed controls: enable/style are native, plus first-pass threshold default; gain/frequency/width ranges, waveform, pulse width, and dry mix remain pending.
- [~] Hard tune detailed controls: enable/style are native, plus first-pass amount/rate/window/source command mappings and source default; fuller editing remains pending.

Priority: partially implemented. The Effects page now has daily presets plus first-pass detailed sliders/style buttons; future Effects work should only go deeper if the missing advanced DSP parameters matter in day-to-day use.

Implemented Effects detail chunk:

- Added model-level `EffectsAmountControl` coverage for reverb, echo, pitch, gender, and megaphone amount sliders.
- Added model-level `EffectsStyleGroup` coverage for reverb, echo, pitch, gender, megaphone, robot, and hard tune style buttons.
- Added `EffectsLayoutPolicy` coverage for wrapped quick-preset cards, compact detail panel widths, and bounded style-group cards/buttons so the `STYLES` panel avoids vertical-letter button wrapping.
- Wired the Effects page to render amount sliders and style button groups backed by typed `PersonalCommand` mappings.

## 6. Lighting

- [x] Dedicated Lighting page.
- [x] Global colour preset buttons via quick themes.
- [x] Fader colours and display style controls: first-pass editor covers all faders plus fader A-D colour pairs and display style buttons.
- [~] Button colours and off-style controls: first-pass editor covers button groups plus daily Cough/Bleep buttons; remaining individual sampler/effects buttons can be added if needed.
- [x] Button group colour controls: editor covers fader mute, effect selector, and effect types groups.
- [x] Simple colour targets: editor covers Global, Accent, and Scribble 1-4.
- [x] Encoder colours: editor covers Reverb, Pitch, Echo, and Gender encoder colour triplets.
- [x] Sampler select colours and off styles: editor covers Sampler Select A/B/C colour triplets and off-style control.
- [x] Animation mode controls: editor covers Simple, Rainbow, Ripple, Retro, and None.
- [x] Animation modifiers and waterfall direction.
- [x] Layout polish for dense Lighting controls: adaptive quick-theme cards and wrapped editor panel rows reduce the cramped left column, fit the four quick themes at an 800px-wide window, keep card heights consistent, keep animation/editor controls in vertical panel flow instead of skinny one-character wrapped columns, and give long fader/button/sampler style actions fixed widths.
- [ ] Load only lighting from profile.

Priority: mostly implemented for daily personal use. Remaining Lighting work is profile-only loading and deeper per-button polish if the current editor feels too broad or too click-heavy.

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
- [x] Active sampler bank selector.
- [x] Play next sample for each pad.
- [x] Stop sample playback.
- [x] Playback mode/order controls: first-pass play/stop mode and random order actions.
- [ ] Add/remove sample controls.
- [~] Sample start/stop percentage controls: safe first-pass slot-0 reset buttons are exposed per bank/pad; richer per-sample editing remains pending.
- [x] Clear sample process error.
- [x] Sampler reset-on-clear setting.
- [x] Sampler fade duration setting.

Priority: partially implemented. The native page now covers safe playback/bank controls, workflow settings, and conservative trim reset controls; file import/removal and richer sample-list editing remain deferred because they have more file/workflow edge cases.

## 8. Profiles and persistence

- [ ] Main profile create/load/save-as/delete controls.
- [~] Mic profile create/load/save-as/delete controls: guarded Mic-page actions exist for a named profile slot with same-action second-click confirmation; a full profile manager remains pending.
- [ ] Effect preset load/save/rename controls.
- [ ] Headphone EQ profile save/load/delete controls.
- [ ] Named personal presets for common routing, lighting, and effect states.
- [x] Clear warning boundaries around destructive profile operations for first-pass mic profile actions.

Priority: medium. Useful once daily control pages are stable, but should be implemented carefully because profile operations are stateful/destructive.

## 9. Device/system settings

- [~] Headphone limiter / ClipGuard controls are exposed through the Mic page.
- [~] Some personal app settings/config editing exists for routing workflows.
- [x] Mute hold duration.
- [x] VC mute also mutes chat mic.
- [x] Monitor-with-FX toggle.
- [x] Lock faders toggle.
- [x] VOD mode setting.
- [x] Headphone EQ full editor: dedicated tab with enabled/preamp and ten-band gain/frequency/Q command controls.
- [x] General device/system settings page: first-pass safe daily controls for mute hold duration, VC/chat mic coupling, monitor-with-FX, fader lock, VOD mode, and reload settings; destructive profile operations remain intentionally omitted.

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
- [ ] Lightweight screenshot/manual QA notes for each new page after local run.
- [ ] A small in-app "About / implemented parity" screen, if the checklist becomes useful in the UI itself.

## Recommended next choices

1. Manual QA of the newly added Mic EQ, guarded Mic profiles, advanced Effects, Headphone EQ, and Sampler pages.
   - The current chunks intentionally favor typed safe controls over exhaustive file/profile managers; screenshots/use will reveal the next best polish pass.

2. Fill the remaining file/workflow-heavy gaps only if they become real workflows.
   - Sampler add/remove samples and full profile managers have more destructive/file-state risk than the daily controls now exposed.

3. Further Lighting/editor UX tweaks.
   - Only if manual testing still shows a specific pain point; prefer custom hex inputs or collapsible sections over more preset button spam.

Implemented routing matrix chunk:

- Added a native web-UI-style input/output matrix on the Config / Routing page.
- Added model-level `RoutingMatrixModel` coverage for Mic/Chat/Music/Game/Console/Line In/System/Samples inputs and Headphones/Broadcast/Chat Mic/Sampler/Line Out outputs.
- Added live route-state indicators from the daemon snapshot so each matrix cell shows `Active`, `Off`, or `Unknown` as a compact centered badge while retaining explicit `On`/`Off` commands.
- Tightened the compact badge layout after screenshot QA: cells now reserve a short fixed-height slot, badges use a bounded width/height, and the UI avoids `centered_and_justified` expansion that made active badges fill entire columns.
- Added a final density pass after the fixed screenshot showed the matrix was still too tall/repetitive: `RoutingMatrixLayoutPolicy` now uses smaller cell slots, shorter badges, fixed-size compact `On` / `Off` buttons, and reduced row/column gaps while preserving explicit route commands.
- Added typed `PersonalCommand::SetRouter(input, output, enabled)` mapping to backend `GoXLRCommand::SetRouter` and UI `On`/`Off` buttons per matrix cell.
- Added `RoutingStateBadge` and `RoutingMatrixLayoutPolicy` model coverage so screenshot-driven state-label polish stays testable without brittle widget snapshots.
- Added named routing preset cards above the matrix for `Broadcast Mix`, `Chat Mic`, and `Line Out Safe`; each preset is a deliberate bundle of explicit `SetRouter(input, output, enabled)` commands instead of an optimistic single-toggle state.

Implemented composite screenshot polish pass:

- Effects `STYLES` now uses bounded wrapped style-group cards and fixed-size style buttons to prevent hard-tune style labels from collapsing into vertical one-letter columns.
- Mic processing panels now use `MicLayoutPolicy` wrapped widths/gaps so compressor and safety controls can move to the next row instead of clipping horizontally.
- Mixer dashboard panels and channel strips now use `MixerLayoutPolicy` wrapping so the page is less dependent on one wide mostly-empty horizontal area.
- Added model-level coverage for the new layout policies and routing preset command bundles.

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


Implemented ordered parity batch:

- Added Mic EQ models and Mic-page controls for mini/full EQ band gain/frequency command helpers.
- Added guarded Mic profile actions for load, save current, save-as, and delete against an explicit named slot, with profile-switching/destructive actions requiring a same-action second click before dispatch.
- Added first-pass advanced Effects DSP actions for reverb decay, echo feedback, pitch character, megaphone post gain, robot threshold, and hard-tune source.
- Added a dedicated Headphone EQ tab with enable/disable, preamp, and ten-band gain/frequency/Q command controls.
- Added a dedicated Sampler tab with bank cards, pad actions, active-bank selection, play/stop mode, random order, play-next, and stop playback controls.
- Added model-level tests for all five requested chunks plus typed `PersonalCommand -> GoXLRCommand` mappings.

## Implementation rule for each chunk

For each new parity chunk:

1. Inspect current backend commands and type enums.
2. Add failing focused tests in `personal-ui/tests/app_model.rs`.
3. Implement minimal model and UI wiring in `personal-ui/src/lib.rs`.
4. Update this checklist and README `Local Change Log`.
5. Run focused tests, then full package checks.
6. Commit only after verification passes and the user chooses to commit.
