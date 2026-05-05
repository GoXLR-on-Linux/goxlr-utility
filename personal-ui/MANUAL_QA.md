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

Open follow-up items:

- Capture fresh screenshots manually or via a working portal/tool path, then append page-specific visual findings here.
- Keep `PARITY_CHECKLIST.md` at `[~]` for screenshot/manual QA until each major page has fresh visual notes or screenshots after the current local run.
