# Personal UI manual QA notes

This file captures lightweight local-run/manual-QA notes for the native personal GoXLR UI. It is intentionally evidence-oriented and should not replace the model-level tests in `tests/app_model.rs`.

## 2026-05-05 15:20 CEST — local run after profile-browser follow-up

Local run state:

- Branch: `personal-native-ui-safety`
- Running binary observed: `target/debug/goxlr-personal-ui`
- PID observed: `1088170`
- Hermes process session: `proc_330d220038a3`
- Recent verification before this note: full personal UI verification passed for the Headphone EQ profile-browser follow-up; full app-model suite reported 122 passing tests.

Screenshot capture attempt:

- Environment: COSMIC Wayland (`XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=COSMIC`).
- Attempted non-interactive capture:
  - `cosmic-screenshot --interactive=false --modal=false --notify=false --save-dir /home/pc/goxlr-utility/personal-ui/manual-qa/screenshots`
- Result: portal returned `Error taking screenshot: Portal request didn't succeed: Other`.
- Follow-up: CLI/non-interactive screenshots are not currently reliable in this session. Use user-provided screenshots or a manually triggered COSMIC screenshot when visual evidence is required.

Pages/features to spot-check manually in the running app:

| Area | What to check | Expected safe behavior |
| --- | --- | --- |
| Dashboard | Personal presets (`Go Live`, `Desktop Focus`, `Late Night`, `FX Panic`) remain visible and compact. | Presets are explicit typed command bundles; `FX Panic` remains visually safety-prioritized. |
| Mixer | Fader A-D assignment/mute cards, monitor-mix controls, submix controls, and live submix labels. | Buttons remain bounded; live state is read-only; mutating controls are typed commands only. |
| Config / Routing | Matrix route-state badges and explicit `On`/`Off` controls. | Badges stay compact and do not stretch into tall columns; no optimistic single-toggle route control. |
| Mic | Setup guide, guarded mic profile actions/browser, and processing/safety panels. | Guarded profile actions require the same-action second click; no fake live mic meter is shown. |
| Effects | Quick presets, amount/style controls, advanced defaults, guarded preset/browser rows. | Cards stay top-aligned/compact; guarded preset actions require confirmation. |
| Lighting | Quick themes, detailed colour controls, guarded lighting profile browser. | Long buttons do not collapse into vertical letters; colour-only profile loads remain guarded. |
| Headphone EQ | Compact 5x2 EQ band grid plus guarded profile panel and discovered `.goxlrHeadphoneProfile` rows. | Band cards remain equal-height; load/save/delete profile actions require confirmation. |
| Sampler | Bank/slot controls, guarded sample file workflow, directory browser, live sample rows, trim presets. | Browser `Use path` only copies paths; add/remove actions are guarded; play actions remain unguarded. |
| System | Live system settings status rows and safe daily settings controls. | Status rows are read-only; profile and settings actions remain separated/guarded. |
| Diagnostics | Connection/status summary and recent app/IPC log. | Page is read-only and does not shell out to journal/system controls. |
| About | Implemented/partial parity summary. | Summary distinguishes completed daily parity from intentionally deferred full managers/editors. |

## 2026-06-25 11:05 CEST — production-readiness stabilization pass

Local run state:

- Branch: `personal-native-ui-safety`
- Running binary observed: `target/debug/goxlr-personal-ui`
- PID observed: `679824`
- Hermes process session: `proc_27be2ca0cb0c`
- Stabilization goal: stop adding broad parity features, harden the current combined chunk, and prepare it for commit/manual production use.

Fresh production-readiness checks:

- Full canonical verification was run after the dashboard label fix in this session: formatting, package check, full personal UI tests, system-tray feature check, and whitespace diff check passed with `134` app-model tests passing.
- Focused ad-hoc verification was also run through a temporary `/tmp/hermes-verify-*` script for the changed `DashboardCopy::active_playback_heading()` behavior; the temporary verifier was removed after passing.
- `cargo clippy -p goxlr-personal-ui --all-targets --features system-tray -- -D warnings` could not run because the apt-managed Rust toolchain on this machine does not currently include the `cargo clippy` subcommand.
- `cargo audit` could not run because the `cargo audit` subcommand is not installed.
- `cargo tree -p goxlr-personal-ui --edges normal --duplicates` was run as a lightweight dependency sanity check; duplicate transitive GUI/Wayland crates exist through upstream `eframe`/`egui`/accessibility stacks and were not changed in this pass.

Production-use posture:

- Device-mutating actions are still routed through typed `PersonalCommand` / `GoXLRCommand` mappings instead of raw command editors.
- Stateful/destructive profile, preset, lighting-profile, and sample-file actions remain guarded by same-action second-click confirmation.
- Known intentionally partial areas remain deferred rather than rushed: waveform/drag-drop sample editing, free-form profile import/rename/location management, hidden Megaphone profile parameters, daemon/journal log tailing, and exhaustive fader/scribble edge cases.

Open follow-up items:

- Capture fresh screenshots manually or via a working portal/tool path, then append page-specific visual findings here.
- Before commit, do one quick hands-on pass through Dashboard, Mixer, Effects, Headphone EQ, Sampler, System, Diagnostics, and About against the running PID above.
- Keep `PARITY_CHECKLIST.md` at `[~]` for screenshot/manual QA until each major page has fresh visual notes or screenshots after the current local run.

## 2026-06-25 — production-launcher final integration polish

Branch/run state:

- Branch: `personal-ui-production-launcher`
- Latest pushed hardening commits at the time of this note:
  - `33418b1a fix(personal-ui): harden active audio routing`
  - `983fb0bd fix(personal-ui): harden guarded profile sample workflows`
  - `11032ff9 fix(personal-ui): clamp headphone eq controls`
- GitHub Actions run `28170810748` passed on macOS, Linux, and Windows after the headphone EQ safety commit.

Integration notes to check during the next visual/manual pass:

| Area | What changed | Manual QA expectation |
| --- | --- | --- |
| Config / Routing | Blank app-match rules are ignored; current-route detection prefers daemon sink names before display labels. | Empty routing rules should not move every stream; streams already on a matching GoXLR sink should not show unnecessary pending moves when the sink description is generic. |
| Profiles | Profile browsers ignore non-files and empty stems like `.goxlr`. | Browser rows should list real named profile files only; no blank-name or directory-backed actions should appear. |
| Sampler | Typed add-file paths use the same supported-audio filter as the sample browser. | Unsupported files such as `.txt` should not create guarded add commands; supported audio paths still should. |
| Headphone EQ | Band gain/frequency/Q commands clamp to safe ranges and sanitize `NaN`/`Infinity`; daily presets have safe-bound invariants. | Presets should still render as before, but command generation should not forward extreme/non-finite EQ values. |

Verification evidence:

- Focused local checks passed for routing, guarded profile/sample workflows, and headphone EQ safety.
- Focused temporary `/tmp/hermes-verify-*` ad-hoc verifiers were run after each changed behavior area and removed after passing.
- Full personal UI tests reached `136` passing tests during these hardening passes.

Remaining manual QA gate:

- Fresh screenshots/page-by-page visual notes are still pending because non-interactive COSMIC Wayland screenshot capture previously failed through the portal.
- Before opening/updating the PR, prefer a short hands-on pass through Config / Routing, Profiles/System, Sampler, and Headphone EQ to confirm the new hardening remains visually discoverable.

## 2026-06-25 — local production launcher install

Installed artifacts:

- `~/.local/bin/goxlr-personal-ui`
- `~/.local/share/applications/goxlr-personal-ui.desktop`

Verification:

- `personal-ui/scripts/install-local.sh` was syntax-checked with `bash -n`.
- The script completed a release build with `--features system-tray` and installed the binary plus desktop entry.
- A Python assertion checked that the installed binary exists and is executable, and that the desktop entry contains the expected name, `Exec`, icon, and categories.

Follow-up production helper coverage:

- `install-local.sh` now supports optional `--autostart` and `--uninstall-autostart` modes for `~/.config/autostart/goxlr-personal-ui.desktop`.
- `smoke-local-install.sh` verifies installed binary, desktop entry, optional autostart entry, and desktop metadata validation when `desktop-file-validate` is available.
- `diagnose-runtime.sh` reports common runtime blockers: missing installed binary, no known GoXLR IPC socket, missing daemon process, unavailable `pactl` / PipeWire-Pulse routing support, and missing personal scene config.
