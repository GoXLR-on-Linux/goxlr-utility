//! Two-way synchronisation between GoXLR state and Voicemeeter parameters.
//!
//! Every link remembers the last GoXLR and Voicemeeter values it saw. When one side moves, the
//! other is updated, and a short 'pending' window stops the stale side from bouncing the old
//! value straight back while the change propagates.

use crate::voicemeeter::config::{
    ButtonLink, ButtonSource, Direction, FaderLink, VmProfile, VolumeSource,
};
use goxlr_ipc::{GoXLRCommand, MixerStatus};
use goxlr_types::MuteState;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const PENDING_TIMEOUT: Duration = Duration::from_millis(500);
const DB_EPSILON: f32 = 0.05;

#[derive(Debug, Default, Clone)]
pub struct LinkState {
    last_goxlr: Option<f32>,
    last_vm: Option<f32>,
    pending_goxlr: Option<(f32, Instant)>,
    pending_vm: Option<(f32, Instant)>,
}

#[derive(Debug, Default, Clone)]
pub struct LinkStates {
    pub faders: Vec<LinkState>,
    pub buttons: Vec<LinkState>,
}

impl LinkStates {
    pub fn new(profile: &VmProfile) -> Self {
        Self {
            faders: vec![LinkState::default(); profile.faders.len()],
            buttons: vec![LinkState::default(); profile.buttons.len()],
        }
    }

    /// Keeps the runtime state of links that didn't change, so editing one link doesn't
    /// re-sync all the others.
    pub fn rebuild(&self, old: &VmProfile, new: &VmProfile) -> Self {
        let keep = |states: &Vec<LinkState>, same: &dyn Fn(usize) -> bool, len: usize| {
            (0..len)
                .map(|i| match states.get(i) {
                    Some(state) if same(i) => state.clone(),
                    _ => LinkState::default(),
                })
                .collect()
        };
        Self {
            faders: keep(
                &self.faders,
                &|i| old.faders.get(i) == new.faders.get(i),
                new.faders.len(),
            ),
            buttons: keep(
                &self.buttons,
                &|i| old.buttons.get(i) == new.buttons.get(i),
                new.buttons.len(),
            ),
        }
    }
}

#[derive(Default)]
pub struct Actions {
    pub vm: Vec<(String, f32)>,
    pub goxlr: Vec<GoXLRCommand>,
}

enum Link<'a> {
    Fader(&'a FaderLink),
    Button(&'a ButtonLink),
}

impl Link<'_> {
    fn target(&self) -> &str {
        match self {
            Link::Fader(link) => &link.target,
            Link::Button(link) => &link.target,
        }
    }

    fn direction(&self) -> Direction {
        match self {
            Link::Fader(link) => link.direction,
            Link::Button(link) => link.direction,
        }
    }

    fn read_goxlr(&self, status: &MixerStatus) -> Option<f32> {
        match self {
            Link::Fader(link) => read_volume(link.source, status).map(f32::from),
            Link::Button(link) => read_button(link.source, status).map(bool_value),
        }
    }

    fn goxlr_to_vm(&self, goxlr: f32) -> f32 {
        match self {
            Link::Fader(link) => link.min_db + (goxlr / 255.0) * (link.max_db - link.min_db),
            Link::Button(link) => bool_value((goxlr >= 0.5) != link.invert),
        }
    }

    fn vm_to_goxlr(&self, vm: f32) -> f32 {
        match self {
            Link::Fader(link) => {
                let range = link.max_db - link.min_db;
                if range.abs() < f32::EPSILON {
                    return 0.0;
                }
                (((vm - link.min_db) / range) * 255.0)
                    .round()
                    .clamp(0.0, 255.0)
            }
            Link::Button(link) => bool_value((vm >= 0.5) != link.invert),
        }
    }

    fn command(&self, goxlr: f32, status: &MixerStatus) -> Option<GoXLRCommand> {
        match self {
            Link::Fader(link) => {
                // A fader link targets whatever channel is currently on that fader
                let channel = match link.source {
                    VolumeSource::Channel(channel) => channel,
                    VolumeSource::Fader(fader) => status.fader_status[fader].channel,
                };
                Some(GoXLRCommand::SetVolume(channel, goxlr as u8))
            }
            Link::Button(link) => write_button(link.source, goxlr >= 0.5),
        }
    }

    fn same_goxlr(&self, a: f32, b: f32) -> bool {
        match self {
            Link::Fader(_) => (a - b).abs() < 0.5,
            Link::Button(_) => (a >= 0.5) == (b >= 0.5),
        }
    }

    fn same_vm(&self, a: f32, b: f32) -> bool {
        match self {
            Link::Fader(_) => (a - b).abs() < DB_EPSILON,
            Link::Button(_) => (a >= 0.5) == (b >= 0.5),
        }
    }
}

fn bool_value(value: bool) -> f32 {
    if value { 1.0 } else { 0.0 }
}

pub fn read_volume(source: VolumeSource, status: &MixerStatus) -> Option<u8> {
    let channel = match source {
        VolumeSource::Channel(channel) => channel,
        VolumeSource::Fader(fader) => status.fader_status[fader].channel,
    };
    Some(status.levels.volumes[channel])
}

pub fn read_button(source: ButtonSource, status: &MixerStatus) -> Option<bool> {
    let effects = status.effects.as_ref();
    match source {
        ButtonSource::FaderMute(fader) => {
            Some(status.fader_status[fader].mute_state != MuteState::Unmuted)
        }
        ButtonSource::CoughMute => Some(status.cough_button.state != MuteState::Unmuted),
        ButtonSource::FxEnabled => effects.map(|e| e.is_enabled),
        ButtonSource::Megaphone => effects.map(|e| e.current.megaphone.is_enabled),
        ButtonSource::Robot => effects.map(|e| e.current.robot.is_enabled),
        ButtonSource::HardTune => effects.map(|e| e.current.hard_tune.is_enabled),
        ButtonSource::EffectPreset(preset) => effects.map(|e| e.active_preset == preset),
        ButtonSource::SamplerBank(bank) => status.sampler.as_ref().map(|s| s.active_bank == bank),
        ButtonSource::ButtonHeld(button) => Some(status.button_down[button]),
    }
}

fn write_button(source: ButtonSource, on: bool) -> Option<GoXLRCommand> {
    let mute = if on {
        MuteState::MutedToX
    } else {
        MuteState::Unmuted
    };
    match source {
        ButtonSource::FaderMute(fader) => Some(GoXLRCommand::SetFaderMuteState(fader, mute)),
        ButtonSource::CoughMute => Some(GoXLRCommand::SetCoughMuteState(mute)),
        ButtonSource::FxEnabled => Some(GoXLRCommand::SetFXEnabled(on)),
        ButtonSource::Megaphone => Some(GoXLRCommand::SetMegaphoneEnabled(on)),
        ButtonSource::Robot => Some(GoXLRCommand::SetRobotEnabled(on)),
        ButtonSource::HardTune => Some(GoXLRCommand::SetHardTuneEnabled(on)),

        // These are 'radio' style, turning one off doesn't mean anything
        ButtonSource::EffectPreset(preset) => {
            on.then_some(GoXLRCommand::SetActiveEffectPreset(preset))
        }
        ButtonSource::SamplerBank(bank) => on.then_some(GoXLRCommand::SetActiveSamplerBank(bank)),

        ButtonSource::ButtonHeld(_) => None,
    }
}

/// Every Voicemeeter parameter a profile needs polled
pub fn targets(profile: &VmProfile) -> impl Iterator<Item = &String> {
    profile
        .faders
        .iter()
        .map(|l| &l.target)
        .chain(profile.buttons.iter().map(|l| &l.target))
}

/// Compares both sides of every link and works out what needs to be sent where. `values` is
/// updated with anything sent to Voicemeeter so later links see the new value.
pub fn evaluate(
    profile: &VmProfile,
    states: &mut LinkStates,
    status: &MixerStatus,
    values: &mut HashMap<String, f32>,
) -> Actions {
    let mut actions = Actions::default();
    let links = profile
        .faders
        .iter()
        .map(Link::Fader)
        .zip(states.faders.iter_mut())
        .chain(
            profile
                .buttons
                .iter()
                .map(Link::Button)
                .zip(states.buttons.iter_mut()),
        );

    for (link, state) in links {
        let target = link.target().trim();
        let (Some(goxlr), Some(&vm)) = (link.read_goxlr(status), values.get(target)) else {
            continue;
        };
        let command = |value| link.command(value, status);
        sync(
            &link,
            state,
            &command,
            target,
            goxlr,
            vm,
            values,
            &mut actions,
        );
    }
    actions
}

#[allow(clippy::too_many_arguments)]
fn sync(
    link: &Link,
    state: &mut LinkState,
    command: &dyn Fn(f32) -> Option<GoXLRCommand>,
    target: &str,
    goxlr: f32,
    vm: f32,
    values: &mut HashMap<String, f32>,
    actions: &mut Actions,
) {
    let now = Instant::now();
    let direction = link.direction();

    if let Some((expected, at)) = state.pending_goxlr
        && (link.same_goxlr(expected, goxlr) || at.elapsed() > PENDING_TIMEOUT)
    {
        state.pending_goxlr = None;
    }
    if let Some((expected, at)) = state.pending_vm
        && (link.same_vm(expected, vm) || at.elapsed() > PENDING_TIMEOUT)
    {
        state.pending_vm = None;
    }

    let initial = state.last_goxlr.is_none() && state.last_vm.is_none();
    let goxlr_changed = state.pending_goxlr.is_none()
        && state
            .last_goxlr
            .is_none_or(|last| !link.same_goxlr(last, goxlr));
    let vm_changed =
        state.pending_vm.is_none() && state.last_vm.is_none_or(|last| !link.same_vm(last, vm));

    if (initial || goxlr_changed) && direction.sends_to_vm() {
        let new_vm = link.goxlr_to_vm(goxlr);
        if !link.same_vm(new_vm, vm) {
            actions.vm.push((target.to_string(), new_vm));
            values.insert(target.to_string(), new_vm);
            state.pending_vm = Some((new_vm, now));
        }
        state.last_goxlr = Some(goxlr);
        state.last_vm = Some(new_vm);
    } else if (initial || vm_changed) && direction.accepts_from_vm() {
        let new_goxlr = link.vm_to_goxlr(vm);
        if !link.same_goxlr(new_goxlr, goxlr)
            && let Some(command) = command(new_goxlr)
        {
            actions.goxlr.push(command);
            state.pending_goxlr = Some((new_goxlr, now));
        }
        state.last_goxlr = Some(new_goxlr);
        state.last_vm = Some(vm);
    } else {
        // Nothing to send in this direction, just keep track of where things are.
        if state.pending_goxlr.is_none() {
            state.last_goxlr = Some(goxlr);
        }
        if state.pending_vm.is_none() {
            state.last_vm = Some(vm);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goxlr_types::ChannelName;

    fn fader_link(direction: Direction) -> FaderLink {
        FaderLink {
            source: VolumeSource::Channel(ChannelName::Mic),
            target: "Strip[0].Gain".into(),
            direction,
            min_db: -60.0,
            max_db: 0.0,
        }
    }

    fn button_link(invert: bool) -> ButtonLink {
        ButtonLink {
            source: ButtonSource::CoughMute,
            target: "Strip[0].Mute".into(),
            direction: Direction::Both,
            invert,
        }
    }

    /// Runs a single sync pass, returning (values sent to Voicemeeter, commands sent to the GoXLR)
    fn step(link: &Link, state: &mut LinkState, goxlr: f32, vm: f32) -> (Vec<f32>, Vec<String>) {
        let mut values = HashMap::from([(link.target().to_string(), vm)]);
        let mut actions = Actions::default();
        let command = |value: f32| match link {
            Link::Fader(_) => Some(GoXLRCommand::SetVolume(ChannelName::Mic, value as u8)),
            Link::Button(l) => write_button(l.source, value >= 0.5),
        };
        sync(
            link,
            state,
            &command,
            link.target(),
            goxlr,
            vm,
            &mut values,
            &mut actions,
        );
        (
            actions.vm.into_iter().map(|(_, v)| v).collect(),
            actions.goxlr.iter().map(|c| format!("{c:?}")).collect(),
        )
    }

    #[test]
    fn initial_sync_goes_to_voicemeeter() {
        let link = fader_link(Direction::Both);
        let link = Link::Fader(&link);
        let mut state = LinkState::default();

        assert_eq!(step(&link, &mut state, 255.0, -20.0), (vec![0.0], vec![]));

        // Voicemeeter hasn't caught up yet, the old value must not be pushed back to the GoXLR
        assert_eq!(step(&link, &mut state, 255.0, -20.0), (vec![], vec![]));
        assert_eq!(step(&link, &mut state, 255.0, 0.0), (vec![], vec![]));
    }

    #[test]
    fn voicemeeter_drag_does_not_bounce() {
        let link = fader_link(Direction::Both);
        let link = Link::Fader(&link);
        let mut state = LinkState::default();
        step(&link, &mut state, 255.0, 0.0);

        // Voicemeeter fader moves, the GoXLR status is still stale for a few passes
        assert_eq!(
            step(&link, &mut state, 255.0, -30.0),
            (vec![], vec!["SetVolume(Mic, 128)".to_string()])
        );
        assert_eq!(
            step(&link, &mut state, 255.0, -31.0),
            (vec![], vec!["SetVolume(Mic, 123)".to_string()])
        );
        assert_eq!(step(&link, &mut state, 255.0, -31.0), (vec![], vec![]));

        // GoXLR catches up, nothing should echo back
        assert_eq!(step(&link, &mut state, 123.0, -31.0), (vec![], vec![]));

        // And the GoXLR side still works afterwards
        assert_eq!(step(&link, &mut state, 0.0, -31.0), (vec![-60.0], vec![]));
    }

    #[test]
    fn goxlr_change_does_not_echo() {
        let link = fader_link(Direction::Both);
        let link = Link::Fader(&link);
        let mut state = LinkState::default();
        step(&link, &mut state, 255.0, 0.0);

        let (vm, goxlr) = step(&link, &mut state, 51.0, 0.0);
        assert_eq!(goxlr, Vec::<String>::new());
        assert!((vm[0] - -48.0).abs() < 0.001);

        // Voicemeeter reports back (slightly rounded), no command to the GoXLR
        assert_eq!(step(&link, &mut state, 51.0, -48.01), (vec![], vec![]));
    }

    #[test]
    fn one_way_links() {
        let to_vm = fader_link(Direction::ToVoicemeeter);
        let to_vm = Link::Fader(&to_vm);
        let mut state = LinkState::default();
        step(&to_vm, &mut state, 255.0, 0.0);
        assert_eq!(step(&to_vm, &mut state, 255.0, -10.0), (vec![], vec![]));

        let from_vm = fader_link(Direction::FromVoicemeeter);
        let from_vm = Link::Fader(&from_vm);
        let mut state = LinkState::default();
        assert_eq!(
            step(&from_vm, &mut state, 0.0, 0.0),
            (vec![], vec!["SetVolume(Mic, 255)".to_string()])
        );
        assert_eq!(step(&from_vm, &mut state, 10.0, 0.0), (vec![], vec![]));
    }

    #[test]
    fn inverted_button() {
        let link = button_link(true);
        let link = Link::Button(&link);
        let mut state = LinkState::default();

        // Cough muted -> Strip unmuted
        assert_eq!(step(&link, &mut state, 1.0, 1.0), (vec![0.0], vec![]));
        assert_eq!(step(&link, &mut state, 1.0, 0.0), (vec![], vec![]));

        // Strip muted in Voicemeeter -> Cough unmuted
        assert_eq!(
            step(&link, &mut state, 1.0, 1.0),
            (vec![], vec!["SetCoughMuteState(Unmuted)".to_string()])
        );
    }

    #[test]
    fn rebuild_keeps_unchanged_links() {
        let mut old = VmProfile::default();
        old.faders.push(fader_link(Direction::Both));
        old.buttons.push(button_link(false));

        let mut states = LinkStates::new(&old);
        states.faders[0].last_goxlr = Some(1.0);
        states.buttons[0].last_goxlr = Some(1.0);

        let mut new = old.clone();
        new.buttons[0].invert = true;
        let rebuilt = states.rebuild(&old, &new);
        assert_eq!(rebuilt.faders[0].last_goxlr, Some(1.0));
        assert_eq!(rebuilt.buttons[0].last_goxlr, None);
    }

    #[test]
    fn profile_format() {
        let text = r#"{
            "faders": [{"source": {"Fader": "A"}, "target": "Strip[2].Gain"}],
            "buttons": [{"source": "FxEnabled", "target": "Bus[0].Mute", "direction": "ToVoicemeeter", "invert": true}]
        }"#;
        let profile: VmProfile = serde_json::from_str(text).unwrap();
        assert!(profile.enabled);
        assert_eq!(profile.faders[0].min_db, -60.0);
        assert_eq!(profile.faders[0].max_db, 0.0);
        assert_eq!(profile.faders[0].direction, Direction::Both);
        assert_eq!(profile.buttons[0].source, ButtonSource::FxEnabled);
    }
}
