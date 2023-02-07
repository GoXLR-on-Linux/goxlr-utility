use crate::files::can_create_new_file;
use crate::profile::ProfileAdapter;
use anyhow::{anyhow, bail, Context, Result};
use byteorder::{ByteOrder, LittleEndian};
use enum_map::EnumMap;
use goxlr_ipc::{Compressor, Equaliser, EqualiserMini, NoiseGate};
use goxlr_profile_loader::mic_profile::MicProfileSettings;
use goxlr_types::{
    CompressorAttackTime, CompressorRatio, CompressorReleaseTime, DisplayMode, EffectKey,
    EqFrequencies, GateTimes, MicrophoneParamKey, MicrophoneType, MiniEqFrequencies,
};
use log::{debug, error};
use ritelinked::LinkedHashSet;
use std::collections::{HashMap, HashSet};
use std::fs::{remove_file, File};
use std::io::{Cursor, Read, Seek};
use std::path::Path;
use strum::IntoEnumIterator;

pub const DEFAULT_MIC_PROFILE_NAME: &str = "DEFAULT";
const DEFAULT_MIC_PROFILE: &[u8] = include_bytes!("../profiles/DEFAULT.goxlrMicProfile");

static GATE_ATTENUATION: [i8; 26] = [
    -6, -7, -8, -9, -10, -11, -12, -13, -14, -15, -16, -17, -18, -19, -20, -21, -22, -23, -24, -25,
    -26, -27, -28, -30, -32, -61,
];

#[derive(Debug)]
pub struct MicProfileAdapter {
    name: String,
    profile: MicProfileSettings,
}

impl MicProfileAdapter {
    pub fn from_named_or_default(name: String, directory: &Path) -> Self {
        match MicProfileAdapter::from_named(name.clone(), directory) {
            Ok(result) => result,
            Err(error) => {
                error!("Couldn't load mic profile {}: {}", name, error);
                MicProfileAdapter::default()
            }
        }
    }

    pub fn from_named(name: String, directory: &Path) -> Result<Self> {
        let path = directory.join(format!("{name}.goxlrMicProfile"));
        if path.is_file() {
            let file = File::open(path).context("Couldn't open mic profile for reading")?;
            return MicProfileAdapter::from_reader(name, file).context("Couldn't read mic profile");
        }
        bail!(
            "Mic Profile {} does not exist inside {}",
            name,
            directory.to_string_lossy()
        );
    }

    pub fn default() -> Self {
        MicProfileAdapter::from_reader(
            DEFAULT_MIC_PROFILE_NAME.to_string(),
            Cursor::new(DEFAULT_MIC_PROFILE),
        )
        .expect("Default mic profile isn't available")
    }

    pub fn from_reader<R: Read + Seek>(name: String, reader: R) -> Result<Self> {
        let profile = MicProfileSettings::load(reader)?;
        Ok(Self { name, profile })
    }

    pub fn can_create_new_file(name: String, directory: &Path) -> Result<()> {
        let path = directory.join(format!("{name}.goxlrMicProfile"));
        can_create_new_file(path)
    }

    pub fn write_profile(&mut self, name: String, directory: &Path, overwrite: bool) -> Result<()> {
        let path = directory.join(format!("{name}.goxlrMicProfile"));
        if !overwrite && path.is_file() {
            return Err(anyhow!("Profile exists, will not overwrite"));
        }

        self.profile.save(path)?;

        // Keep our names in sync (in case it was changed)
        if name != self.name() {
            debug!("Changing Profile Name: {} -> {}", self.name(), name);
            self.name = name;
        }

        Ok(())
    }

    pub fn delete_profile(&mut self, name: String, directory: &Path) -> Result<()> {
        let path = directory.join(format!("{name}.goxlrMicProfile"));
        if path.is_file() {
            remove_file(path)?;
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mic_gains(&self) -> EnumMap<MicrophoneType, u16> {
        let mut gains = EnumMap::default();
        gains[MicrophoneType::Condenser] = self.profile.setup().condenser_mic_gain();
        gains[MicrophoneType::Dynamic] = self.profile.setup().dynamic_mic_gain();
        gains[MicrophoneType::Jack] = self.profile.setup().trs_mic_gain();

        gains
    }

    pub fn mic_type(&self) -> MicrophoneType {
        match self.profile.setup().mic_type() {
            0 => MicrophoneType::Dynamic,
            1 => MicrophoneType::Condenser,
            2 => MicrophoneType::Jack,
            _ => MicrophoneType::Jack, // default
        }
    }

    pub fn get_gate_display_mode(&self) -> DisplayMode {
        if self.profile.ui_setup().gate_advanced() {
            DisplayMode::Advanced
        } else {
            DisplayMode::Simple
        }
    }

    pub fn set_gate_display_mode(&mut self, display_mode: DisplayMode) {
        self.profile
            .ui_setup_mut()
            .set_gate_advanced(display_mode != DisplayMode::Simple);
    }

    pub fn get_compressor_display_mode(&self) -> DisplayMode {
        if self.profile.ui_setup().comp_advanced() {
            DisplayMode::Advanced
        } else {
            DisplayMode::Simple
        }
    }

    pub fn set_compressor_display_mode(&mut self, display_mode: DisplayMode) {
        self.profile
            .ui_setup_mut()
            .set_comp_advanced(display_mode != DisplayMode::Simple);
    }

    pub fn get_eq_display_mode(&self) -> DisplayMode {
        if self.profile.ui_setup().eq_advanced() {
            DisplayMode::Advanced
        } else {
            DisplayMode::Simple
        }
    }

    pub fn set_eq_display_mode(&mut self, display_mode: DisplayMode) {
        self.profile
            .ui_setup_mut()
            .set_eq_advanced(display_mode != DisplayMode::Simple);
    }

    pub fn get_eq_fine_display_mode(&self) -> DisplayMode {
        if self.profile.ui_setup().eq_fine_tune() {
            DisplayMode::Advanced
        } else {
            DisplayMode::Simple
        }
    }

    pub fn set_eq_fine_display_mode(&mut self, display_mode: DisplayMode) {
        self.profile
            .ui_setup_mut()
            .set_eq_fine_tune(display_mode != DisplayMode::Simple);
    }

    pub fn noise_gate_ipc(&self) -> NoiseGate {
        NoiseGate {
            threshold: self.profile.gate().threshold(),
            attack: GateTimes::iter()
                .nth(self.profile.gate().attack() as usize)
                .unwrap(),
            release: GateTimes::iter()
                .nth(self.profile.gate().release() as usize)
                .unwrap(),
            enabled: self.profile.gate().enabled(),
            attenuation: self.profile.gate().attenuation(),
        }
    }

    pub fn compressor_ipc(&self) -> Compressor {
        Compressor {
            threshold: self.profile.compressor().threshold(),
            ratio: CompressorRatio::iter()
                .nth(self.profile.compressor().ratio() as usize)
                .unwrap(),
            attack: CompressorAttackTime::iter()
                .nth(self.profile.compressor().attack() as usize)
                .unwrap(),
            release: CompressorReleaseTime::iter()
                .nth(self.profile.compressor().release() as usize)
                .unwrap(),
            makeup_gain: self.profile.compressor().makeup(),
        }
    }

    pub fn equalizer_ipc(&self) -> Equaliser {
        let mut gains: HashMap<EqFrequencies, i8> = Default::default();
        for freq in EqFrequencies::iter() {
            gains.insert(freq, self.get_eq_gain(freq));
        }

        let mut freqs: HashMap<EqFrequencies, f32> = Default::default();
        for freq in EqFrequencies::iter() {
            freqs.insert(freq, self.get_eq_freq(freq));
        }

        Equaliser {
            gain: gains,
            frequency: freqs,
        }
    }

    pub fn equalizer_mini_ipc(&self) -> EqualiserMini {
        let mut gains: HashMap<MiniEqFrequencies, i8> = Default::default();
        for freq in MiniEqFrequencies::iter() {
            gains.insert(freq, self.get_mini_eq_gain(freq));
        }

        let mut freqs: HashMap<MiniEqFrequencies, f32> = Default::default();
        for freq in MiniEqFrequencies::iter() {
            freqs.insert(freq, self.get_mini_eq_freq(freq));
        }

        EqualiserMini {
            gain: gains,
            frequency: freqs,
        }
    }

    pub fn set_mic_type(&mut self, mic_type: MicrophoneType) -> Result<()> {
        self.profile.setup_mut().set_mic_type(mic_type as u8)
    }

    pub fn set_mic_gain(&mut self, mic_type: MicrophoneType, gain: u16) -> Result<()> {
        match mic_type {
            MicrophoneType::Dynamic => self.profile.setup_mut().set_dynamic_mic_gain(gain)?,
            MicrophoneType::Condenser => self.profile.setup_mut().set_condenser_mic_gain(gain)?,
            MicrophoneType::Jack => self.profile.setup_mut().set_trs_mic_gain(gain)?,
        }
        Ok(())
    }

    pub fn set_eq_gain(&mut self, gain: EqFrequencies, value: i8) -> Result<EffectKey> {
        match gain {
            EqFrequencies::Equalizer31Hz => {
                self.profile.equalizer_mut().set_eq_31h_gain(value)?;
                Ok(EffectKey::Equalizer31HzGain)
            }
            EqFrequencies::Equalizer63Hz => {
                self.profile.equalizer_mut().set_eq_63h_gain(value)?;
                Ok(EffectKey::Equalizer63HzGain)
            }
            EqFrequencies::Equalizer125Hz => {
                self.profile.equalizer_mut().set_eq_125h_gain(value)?;
                Ok(EffectKey::Equalizer125HzGain)
            }
            EqFrequencies::Equalizer250Hz => {
                self.profile.equalizer_mut().set_eq_250h_gain(value)?;
                Ok(EffectKey::Equalizer250HzGain)
            }
            EqFrequencies::Equalizer500Hz => {
                self.profile.equalizer_mut().set_eq_500h_gain(value)?;
                Ok(EffectKey::Equalizer500HzGain)
            }
            EqFrequencies::Equalizer1KHz => {
                self.profile.equalizer_mut().set_eq_1k_gain(value)?;
                Ok(EffectKey::Equalizer1KHzGain)
            }
            EqFrequencies::Equalizer2KHz => {
                self.profile.equalizer_mut().set_eq_2k_gain(value)?;
                Ok(EffectKey::Equalizer2KHzGain)
            }
            EqFrequencies::Equalizer4KHz => {
                self.profile.equalizer_mut().set_eq_4k_gain(value)?;
                Ok(EffectKey::Equalizer4KHzGain)
            }
            EqFrequencies::Equalizer8KHz => {
                self.profile.equalizer_mut().set_eq_8k_gain(value)?;
                Ok(EffectKey::Equalizer8KHzGain)
            }
            EqFrequencies::Equalizer16KHz => {
                self.profile.equalizer_mut().set_eq_16k_gain(value)?;
                Ok(EffectKey::Equalizer16KHzGain)
            }
        }
    }

    pub fn get_eq_gain(&self, freq: EqFrequencies) -> i8 {
        let eq = self.profile.equalizer();
        match freq {
            EqFrequencies::Equalizer31Hz => eq.eq_31h_gain(),
            EqFrequencies::Equalizer63Hz => eq.eq_63h_gain(),
            EqFrequencies::Equalizer125Hz => eq.eq_125h_gain(),
            EqFrequencies::Equalizer250Hz => eq.eq_250h_gain(),
            EqFrequencies::Equalizer500Hz => eq.eq_500h_gain(),
            EqFrequencies::Equalizer1KHz => eq.eq_1k_gain(),
            EqFrequencies::Equalizer2KHz => eq.eq_2k_gain(),
            EqFrequencies::Equalizer4KHz => eq.eq_4k_gain(),
            EqFrequencies::Equalizer8KHz => eq.eq_8k_gain(),
            EqFrequencies::Equalizer16KHz => eq.eq_16k_gain(),
        }
    }

    pub fn set_eq_freq(&mut self, freq: EqFrequencies, value: f32) -> Result<EffectKey> {
        match freq {
            EqFrequencies::Equalizer31Hz => {
                self.profile.equalizer_mut().set_eq_31h_freq(value)?;
                Ok(EffectKey::Equalizer31HzFrequency)
            }
            EqFrequencies::Equalizer63Hz => {
                self.profile.equalizer_mut().set_eq_63h_freq(value)?;
                Ok(EffectKey::Equalizer63HzFrequency)
            }
            EqFrequencies::Equalizer125Hz => {
                self.profile.equalizer_mut().set_eq_125h_freq(value)?;
                Ok(EffectKey::Equalizer125HzFrequency)
            }
            EqFrequencies::Equalizer250Hz => {
                self.profile.equalizer_mut().set_eq_250h_freq(value)?;
                Ok(EffectKey::Equalizer250HzFrequency)
            }
            EqFrequencies::Equalizer500Hz => {
                self.profile.equalizer_mut().set_eq_500h_freq(value)?;
                Ok(EffectKey::Equalizer500HzFrequency)
            }
            EqFrequencies::Equalizer1KHz => {
                self.profile.equalizer_mut().set_eq_1k_freq(value)?;
                Ok(EffectKey::Equalizer1KHzFrequency)
            }
            EqFrequencies::Equalizer2KHz => {
                self.profile.equalizer_mut().set_eq_2k_freq(value)?;
                Ok(EffectKey::Equalizer2KHzFrequency)
            }
            EqFrequencies::Equalizer4KHz => {
                self.profile.equalizer_mut().set_eq_4k_freq(value)?;
                Ok(EffectKey::Equalizer4KHzFrequency)
            }
            EqFrequencies::Equalizer8KHz => {
                self.profile.equalizer_mut().set_eq_8k_freq(value)?;
                Ok(EffectKey::Equalizer8KHzFrequency)
            }
            EqFrequencies::Equalizer16KHz => {
                self.profile.equalizer_mut().set_eq_16k_freq(value)?;
                Ok(EffectKey::Equalizer16KHzFrequency)
            }
        }
    }

    pub fn get_eq_freq(&self, freq: EqFrequencies) -> f32 {
        let eq = self.profile.equalizer();
        match freq {
            EqFrequencies::Equalizer31Hz => eq.eq_31h_freq(),
            EqFrequencies::Equalizer63Hz => eq.eq_63h_freq(),
            EqFrequencies::Equalizer125Hz => eq.eq_125h_freq(),
            EqFrequencies::Equalizer250Hz => eq.eq_250h_freq(),
            EqFrequencies::Equalizer500Hz => eq.eq_500h_freq(),
            EqFrequencies::Equalizer1KHz => eq.eq_1k_freq(),
            EqFrequencies::Equalizer2KHz => eq.eq_2k_freq(),
            EqFrequencies::Equalizer4KHz => eq.eq_4k_freq(),
            EqFrequencies::Equalizer8KHz => eq.eq_8k_freq(),
            EqFrequencies::Equalizer16KHz => eq.eq_16k_freq(),
        }
    }

    pub fn set_mini_eq_gain(
        &mut self,
        gain: MiniEqFrequencies,
        value: i8,
    ) -> Result<MicrophoneParamKey> {
        match gain {
            MiniEqFrequencies::Equalizer90Hz => {
                self.profile.equalizer_mini_mut().set_eq_90h_gain(value)?;
                Ok(MicrophoneParamKey::Equalizer90HzGain)
            }
            MiniEqFrequencies::Equalizer250Hz => {
                self.profile.equalizer_mini_mut().set_eq_250h_gain(value)?;
                Ok(MicrophoneParamKey::Equalizer250HzGain)
            }
            MiniEqFrequencies::Equalizer500Hz => {
                self.profile.equalizer_mini_mut().set_eq_500h_gain(value)?;
                Ok(MicrophoneParamKey::Equalizer500HzGain)
            }
            MiniEqFrequencies::Equalizer1KHz => {
                self.profile.equalizer_mini_mut().set_eq_1k_gain(value)?;
                Ok(MicrophoneParamKey::Equalizer1KHzGain)
            }
            MiniEqFrequencies::Equalizer3KHz => {
                self.profile.equalizer_mini_mut().set_eq_3k_gain(value)?;
                Ok(MicrophoneParamKey::Equalizer3KHzGain)
            }
            MiniEqFrequencies::Equalizer8KHz => {
                self.profile.equalizer_mini_mut().set_eq_8k_gain(value)?;
                Ok(MicrophoneParamKey::Equalizer8KHzGain)
            }
        }
    }

    pub fn get_mini_eq_gain(&self, gain: MiniEqFrequencies) -> i8 {
        let eq = self.profile.equalizer_mini();
        match gain {
            MiniEqFrequencies::Equalizer90Hz => eq.eq_90h_gain(),
            MiniEqFrequencies::Equalizer250Hz => eq.eq_250h_gain(),
            MiniEqFrequencies::Equalizer500Hz => eq.eq_500h_gain(),
            MiniEqFrequencies::Equalizer1KHz => eq.eq_1k_gain(),
            MiniEqFrequencies::Equalizer3KHz => eq.eq_3k_gain(),
            MiniEqFrequencies::Equalizer8KHz => eq.eq_8k_gain(),
        }
    }

    pub fn set_mini_eq_freq(
        &mut self,
        freq: MiniEqFrequencies,
        value: f32,
    ) -> Result<MicrophoneParamKey> {
        match freq {
            MiniEqFrequencies::Equalizer90Hz => {
                self.profile.equalizer_mini_mut().set_eq_90h_freq(value)?;
                Ok(MicrophoneParamKey::Equalizer90HzFrequency)
            }
            MiniEqFrequencies::Equalizer250Hz => {
                self.profile.equalizer_mini_mut().set_eq_250h_freq(value)?;
                Ok(MicrophoneParamKey::Equalizer250HzFrequency)
            }
            MiniEqFrequencies::Equalizer500Hz => {
                self.profile.equalizer_mini_mut().set_eq_500h_freq(value)?;
                Ok(MicrophoneParamKey::Equalizer500HzFrequency)
            }
            MiniEqFrequencies::Equalizer1KHz => {
                self.profile.equalizer_mini_mut().set_eq_1k_freq(value)?;
                Ok(MicrophoneParamKey::Equalizer1KHzFrequency)
            }
            MiniEqFrequencies::Equalizer3KHz => {
                self.profile.equalizer_mini_mut().set_eq_3k_freq(value)?;
                Ok(MicrophoneParamKey::Equalizer3KHzFrequency)
            }
            MiniEqFrequencies::Equalizer8KHz => {
                self.profile.equalizer_mini_mut().set_eq_8k_freq(value)?;
                Ok(MicrophoneParamKey::Equalizer8KHzFrequency)
            }
        }
    }

    pub fn get_mini_eq_freq(&self, freq: MiniEqFrequencies) -> f32 {
        let eq = self.profile.equalizer_mini();
        match freq {
            MiniEqFrequencies::Equalizer90Hz => eq.eq_90h_freq(),
            MiniEqFrequencies::Equalizer250Hz => eq.eq_250h_freq(),
            MiniEqFrequencies::Equalizer500Hz => eq.eq_500h_freq(),
            MiniEqFrequencies::Equalizer1KHz => eq.eq_1k_freq(),
            MiniEqFrequencies::Equalizer3KHz => eq.eq_3k_freq(),
            MiniEqFrequencies::Equalizer8KHz => eq.eq_8k_freq(),
        }
    }

    pub fn set_gate_threshold(&mut self, value: i8) -> Result<()> {
        self.profile.gate_mut().set_threshold(value)
    }

    pub fn set_gate_attenuation(&mut self, value: u8) -> Result<()> {
        self.profile.gate_mut().set_attenuation(value)
    }

    pub fn set_gate_attack(&mut self, value: GateTimes) -> Result<()> {
        self.profile.gate_mut().set_attack(value as u8)
    }

    pub fn set_gate_release(&mut self, value: GateTimes) -> Result<()> {
        self.profile.gate_mut().set_release(value as u8)
    }

    pub fn set_gate_active(&mut self, value: bool) -> Result<()> {
        self.profile.gate_mut().set_enabled(value)
    }

    pub fn set_compressor_threshold(&mut self, value: i8) -> Result<()> {
        self.profile.compressor_mut().set_threshold(value)
    }

    pub fn set_compressor_ratio(&mut self, value: CompressorRatio) -> Result<()> {
        self.profile.compressor_mut().set_ratio(value as u8)
    }

    pub fn set_compressor_attack(&mut self, value: CompressorAttackTime) -> Result<()> {
        self.profile.compressor_mut().set_attack(value as u8)
    }

    pub fn set_compressor_release(&mut self, value: CompressorReleaseTime) -> Result<()> {
        self.profile.compressor_mut().set_release(value as u8)
    }

    pub fn set_compressor_makeup(&mut self, value: i8) -> Result<()> {
        self.profile.compressor_mut().set_makeup_gain(value)
    }

    pub fn set_deesser(&mut self, value: u8) -> Result<()> {
        self.profile.set_deess(value)
    }

    pub fn set_bleep_level(&mut self, value: i8) -> Result<()> {
        self.profile.set_bleep_level(value)
    }

    pub fn bleep_level(&self) -> i8 {
        self.profile.bleep_level()
    }

    /// The uber method, fetches the relevant setting from the profile and returns it..
    pub fn get_param_value(&self, param: MicrophoneParamKey) -> [u8; 4] {
        let gains = self.mic_gains();

        match param {
            MicrophoneParamKey::MicType => {
                let microphone_type: MicrophoneType = self.mic_type();
                match microphone_type.has_phantom_power() {
                    true => [0x01_u8, 0, 0, 0],
                    false => [0, 0, 0, 0],
                }
            }
            MicrophoneParamKey::DynamicGain => self.gain_value(gains[MicrophoneType::Dynamic]),
            MicrophoneParamKey::CondenserGain => self.gain_value(gains[MicrophoneType::Condenser]),
            MicrophoneParamKey::JackGain => self.gain_value(gains[MicrophoneType::Jack]),
            MicrophoneParamKey::GateThreshold => self.i8_to_f32(self.profile.gate().threshold()),
            MicrophoneParamKey::GateAttack => self.u8_to_f32(self.profile.gate().attack()),
            MicrophoneParamKey::GateRelease => self.u8_to_f32(self.profile.gate().release()),
            MicrophoneParamKey::GateAttenuation => self
                .i8_to_f32(self.gate_attenuation_from_percent(self.profile.gate().attenuation())),
            MicrophoneParamKey::CompressorThreshold => {
                self.i8_to_f32(self.profile.compressor().threshold())
            }
            MicrophoneParamKey::CompressorRatio => {
                self.u8_to_f32(self.profile.compressor().ratio())
            }
            MicrophoneParamKey::CompressorAttack => {
                self.u8_to_f32(self.profile.compressor().ratio())
            }
            MicrophoneParamKey::CompressorRelease => {
                self.u8_to_f32(self.profile.compressor().release())
            }
            MicrophoneParamKey::CompressorMakeUpGain => {
                self.i8_to_f32(self.profile.compressor().makeup())
            }
            MicrophoneParamKey::BleepLevel => self.calculate_bleep(self.profile.bleep_level()),
            MicrophoneParamKey::Equalizer90HzFrequency => {
                self.f32_to_f32(self.profile.equalizer_mini().eq_90h_freq())
            }
            MicrophoneParamKey::Equalizer90HzGain => {
                self.i8_to_f32(self.profile.equalizer_mini().eq_90h_gain())
            }
            MicrophoneParamKey::Equalizer250HzFrequency => {
                self.f32_to_f32(self.profile.equalizer_mini().eq_250h_freq())
            }
            MicrophoneParamKey::Equalizer250HzGain => {
                self.i8_to_f32(self.profile.equalizer_mini().eq_250h_gain())
            }
            MicrophoneParamKey::Equalizer500HzFrequency => {
                self.f32_to_f32(self.profile.equalizer_mini().eq_500h_freq())
            }
            MicrophoneParamKey::Equalizer500HzGain => {
                self.i8_to_f32(self.profile.equalizer_mini().eq_500h_gain())
            }
            MicrophoneParamKey::Equalizer1KHzFrequency => {
                self.f32_to_f32(self.profile.equalizer_mini().eq_1k_freq())
            }
            MicrophoneParamKey::Equalizer1KHzGain => {
                self.i8_to_f32(self.profile.equalizer_mini().eq_1k_gain())
            }
            MicrophoneParamKey::Equalizer3KHzFrequency => {
                self.f32_to_f32(self.profile.equalizer_mini().eq_3k_freq())
            }
            MicrophoneParamKey::Equalizer3KHzGain => {
                self.i8_to_f32(self.profile.equalizer_mini().eq_3k_gain())
            }
            MicrophoneParamKey::Equalizer8KHzFrequency => {
                self.f32_to_f32(self.profile.equalizer_mini().eq_8k_freq())
            }
            MicrophoneParamKey::Equalizer8KHzGain => {
                self.i8_to_f32(self.profile.equalizer_mini().eq_8k_gain())
            }
        }
    }

    fn calculate_bleep(&self, value: i8) -> [u8; 4] {
        let mut return_value = [0; 4];
        LittleEndian::write_f32(&mut return_value, value as f32 * 65536.0);
        return_value
    }

    /// This is going to require a CRAPLOAD of work to sort..
    pub fn get_effect_value(&self, effect: EffectKey, main_profile: &ProfileAdapter) -> i32 {
        match effect {
            EffectKey::DisableMic => {
                // TODO: Actually use this..
                // Originally I favoured just muting the mic channel, but discovered during testing
                // of the effects that the mic is still read even when the channel is muted, so we
                // need to correctly send this when the mic gets muted / unmuted.
                0
            }
            EffectKey::BleepLevel => self.profile.bleep_level().into(),
            EffectKey::GateMode => self.profile.gate_mode().into(),
            EffectKey::GateEnabled => 1, // Used for 'Mic Testing' in the UI
            EffectKey::GateThreshold => self.profile.gate().threshold().into(),
            EffectKey::GateAttenuation => self
                .gate_attenuation_from_percent(self.profile.gate().attenuation())
                .into(),
            EffectKey::GateAttack => self.profile.gate().attack().into(),
            EffectKey::GateRelease => self.profile.gate().release().into(),
            EffectKey::MicCompSelect => self.profile.comp_select().into(),

            EffectKey::Equalizer31HzFrequency => self.profile.equalizer().eq_31h_freq_as_goxlr(),
            EffectKey::Equalizer63HzFrequency => self.profile.equalizer().eq_63h_freq_as_goxlr(),
            EffectKey::Equalizer125HzFrequency => self.profile.equalizer().eq_125h_freq_as_goxlr(),
            EffectKey::Equalizer250HzFrequency => self.profile.equalizer().eq_250h_freq_as_goxlr(),
            EffectKey::Equalizer500HzFrequency => self.profile.equalizer().eq_500h_freq_as_goxlr(),
            EffectKey::Equalizer1KHzFrequency => self.profile.equalizer().eq_1k_freq_as_goxlr(),
            EffectKey::Equalizer2KHzFrequency => self.profile.equalizer().eq_2k_freq_as_goxlr(),
            EffectKey::Equalizer4KHzFrequency => self.profile.equalizer().eq_4k_freq_as_goxlr(),
            EffectKey::Equalizer8KHzFrequency => self.profile.equalizer().eq_8k_freq_as_goxlr(),
            EffectKey::Equalizer16KHzFrequency => self.profile.equalizer().eq_16k_freq_as_goxlr(),

            EffectKey::Equalizer31HzGain => self.profile.equalizer().eq_31h_gain().into(),
            EffectKey::Equalizer63HzGain => self.profile.equalizer().eq_63h_gain().into(),
            EffectKey::Equalizer125HzGain => self.profile.equalizer().eq_125h_gain().into(),
            EffectKey::Equalizer250HzGain => self.profile.equalizer().eq_250h_gain().into(),
            EffectKey::Equalizer500HzGain => self.profile.equalizer().eq_500h_gain().into(),
            EffectKey::Equalizer1KHzGain => self.profile.equalizer().eq_1k_gain().into(),
            EffectKey::Equalizer2KHzGain => self.profile.equalizer().eq_2k_gain().into(),
            EffectKey::Equalizer4KHzGain => self.profile.equalizer().eq_4k_gain().into(),
            EffectKey::Equalizer8KHzGain => self.profile.equalizer().eq_8k_gain().into(),
            EffectKey::Equalizer16KHzGain => self.profile.equalizer().eq_16k_gain().into(),

            EffectKey::CompressorThreshold => self.profile.compressor().threshold().into(),
            EffectKey::CompressorRatio => self.profile.compressor().ratio().into(),
            EffectKey::CompressorAttack => self.profile.compressor().attack().into(),
            EffectKey::CompressorRelease => self.profile.compressor().release().into(),
            EffectKey::CompressorMakeUpGain => self.profile.compressor().makeup().into(),

            EffectKey::DeEsser => self.profile.deess() as i32,

            EffectKey::ReverbAmount => main_profile.get_active_reverb_profile().amount().into(),
            EffectKey::ReverbDecay => main_profile.get_active_reverb_profile().decay().into(),
            EffectKey::ReverbEarlyLevel => main_profile
                .get_active_reverb_profile()
                .early_level()
                .into(),
            EffectKey::ReverbTailLevel => 0, // Always 0 from the Windows UI
            EffectKey::ReverbPredelay => main_profile.get_active_reverb_profile().predelay().into(),
            EffectKey::ReverbLowColor => {
                main_profile.get_active_reverb_profile().low_color().into()
            }
            EffectKey::ReverbHighColor => {
                main_profile.get_active_reverb_profile().high_color().into()
            }
            EffectKey::ReverbHighFactor => {
                main_profile.get_active_reverb_profile().hifactor().into()
            }
            EffectKey::ReverbDiffuse => main_profile.get_active_reverb_profile().diffuse().into(),
            EffectKey::ReverbModSpeed => {
                main_profile.get_active_reverb_profile().mod_speed().into()
            }
            EffectKey::ReverbModDepth => {
                main_profile.get_active_reverb_profile().mod_depth().into()
            }
            EffectKey::ReverbType => main_profile
                .get_active_reverb_profile()
                .reverb_type()
                .into(),

            EffectKey::EchoAmount => main_profile.get_active_echo_profile().amount().into(),
            EffectKey::EchoFeedback => main_profile
                .get_active_echo_profile()
                .feedback_control()
                .into(),
            EffectKey::EchoTempo => main_profile.get_active_echo_profile().tempo().into(),
            EffectKey::EchoDelayL => main_profile.get_active_echo_profile().time_left().into(),
            EffectKey::EchoDelayR => main_profile.get_active_echo_profile().time_right().into(),
            EffectKey::EchoFeedbackL => main_profile
                .get_active_echo_profile()
                .feedback_left()
                .into(),
            EffectKey::EchoXFBLtoR => main_profile.get_active_echo_profile().xfb_l_to_r().into(),
            EffectKey::EchoFeedbackR => main_profile
                .get_active_echo_profile()
                .feedback_right()
                .into(),
            EffectKey::EchoXFBRtoL => main_profile.get_active_echo_profile().xfb_r_to_l().into(),
            EffectKey::EchoSource => main_profile.get_active_echo_profile().source() as i32,
            EffectKey::EchoDivL => main_profile.get_active_echo_profile().div_l().into(),
            EffectKey::EchoDivR => main_profile.get_active_echo_profile().div_r().into(),
            EffectKey::EchoFilterStyle => {
                main_profile.get_active_echo_profile().filter_style().into()
            }

            EffectKey::PitchAmount => main_profile
                .get_active_pitch_profile()
                .get_pitch_value()
                .into(),
            EffectKey::PitchThreshold => main_profile.get_active_pitch_profile().threshold().into(),
            EffectKey::PitchCharacter => main_profile
                .get_active_pitch_profile()
                .inst_ratio_value()
                .into(),

            EffectKey::GenderAmount => main_profile.get_active_gender_profile().amount().into(),

            EffectKey::MegaphoneAmount => main_profile
                .get_active_megaphone_profile()
                .trans_dist_amt()
                .into(),
            EffectKey::MegaphonePostGain => main_profile
                .get_active_megaphone_profile()
                .trans_postgain()
                .into(),
            EffectKey::MegaphoneStyle => {
                *main_profile.get_active_megaphone_profile().style() as i32
            }
            EffectKey::MegaphoneHP => main_profile
                .get_active_megaphone_profile()
                .trans_hp()
                .into(),
            EffectKey::MegaphoneLP => main_profile
                .get_active_megaphone_profile()
                .trans_lp()
                .into(),
            EffectKey::MegaphonePreGain => main_profile
                .get_active_megaphone_profile()
                .trans_pregain()
                .into(),
            EffectKey::MegaphoneDistType => main_profile
                .get_active_megaphone_profile()
                .trans_dist_type()
                .into(),
            EffectKey::MegaphonePresenceGain => main_profile
                .get_active_megaphone_profile()
                .trans_presence_gain()
                .into(),
            EffectKey::MegaphonePresenceFC => main_profile
                .get_active_megaphone_profile()
                .trans_presence_fc()
                .into(),
            EffectKey::MegaphonePresenceBW => main_profile
                .get_active_megaphone_profile()
                .trans_presence_bw()
                .into(),
            EffectKey::MegaphoneBeatboxEnable => main_profile
                .get_active_megaphone_profile()
                .trans_beatbox_enabled()
                .into(),
            EffectKey::MegaphoneFilterControl => main_profile
                .get_active_megaphone_profile()
                .trans_filter_control()
                .into(),
            EffectKey::MegaphoneFilter => main_profile
                .get_active_megaphone_profile()
                .trans_filter()
                .into(),
            EffectKey::MegaphoneDrivePotGainCompMid => main_profile
                .get_active_megaphone_profile()
                .trans_drive_pot_gain_comp_mid()
                .into(),
            EffectKey::MegaphoneDrivePotGainCompMax => main_profile
                .get_active_megaphone_profile()
                .trans_drive_pot_gain_comp_max()
                .into(),

            EffectKey::HardTuneAmount => main_profile.get_active_hardtune_profile().amount().into(),
            EffectKey::HardTuneKeySource => 0, // Always 0, HardTune is handled through routing
            EffectKey::HardTuneScale => main_profile.get_active_hardtune_profile().scale().into(),
            EffectKey::HardTunePitchAmount => main_profile
                .get_active_hardtune_profile()
                .pitch_amt()
                .into(),
            EffectKey::HardTuneRate => main_profile.get_active_hardtune_profile().rate().into(),
            EffectKey::HardTuneWindow => main_profile.get_active_hardtune_profile().window().into(),

            EffectKey::RobotLowGain => main_profile
                .get_active_robot_profile()
                .vocoder_low_gain()
                .into(),
            EffectKey::RobotLowFreq => main_profile
                .get_active_robot_profile()
                .vocoder_low_freq()
                .into(),
            EffectKey::RobotLowWidth => main_profile
                .get_active_robot_profile()
                .vocoder_low_bw()
                .into(),
            EffectKey::RobotMidGain => main_profile
                .get_active_robot_profile()
                .vocoder_mid_gain()
                .into(),
            EffectKey::RobotMidFreq => main_profile
                .get_active_robot_profile()
                .vocoder_mid_freq()
                .into(),
            EffectKey::RobotMidWidth => main_profile
                .get_active_robot_profile()
                .vocoder_mid_bw()
                .into(),
            EffectKey::RobotHiGain => main_profile
                .get_active_robot_profile()
                .vocoder_high_gain()
                .into(),
            EffectKey::RobotHiFreq => main_profile
                .get_active_robot_profile()
                .vocoder_high_freq()
                .into(),
            EffectKey::RobotHiWidth => main_profile
                .get_active_robot_profile()
                .vocoder_high_bw()
                .into(),
            EffectKey::RobotWaveform => main_profile
                .get_active_robot_profile()
                .synthosc_waveform()
                .into(),
            EffectKey::RobotPulseWidth => main_profile
                .get_active_robot_profile()
                .synthosc_pulse_width()
                .into(),
            EffectKey::RobotThreshold => main_profile
                .get_active_robot_profile()
                .vocoder_gate_threshold()
                .into(),
            EffectKey::RobotDryMix => main_profile.get_active_robot_profile().dry_mix().into(),
            EffectKey::RobotStyle => *main_profile.get_active_robot_profile().style() as i32,

            EffectKey::RobotEnabled => main_profile.is_robot_enabled(false).into(),
            EffectKey::MegaphoneEnabled => main_profile.is_megaphone_enabled(false).into(),
            EffectKey::HardTuneEnabled => main_profile.is_hardtune_enabled(false).into(),

            // Encoders are always enabled when FX is enabled..
            EffectKey::Encoder1Enabled => main_profile.is_fx_enabled().into(),
            EffectKey::Encoder2Enabled => main_profile.is_fx_enabled().into(),
            EffectKey::Encoder3Enabled => main_profile.is_fx_enabled().into(),
            EffectKey::Encoder4Enabled => main_profile.is_fx_enabled().into(),
        }
    }

    fn u8_to_f32(&self, value: u8) -> [u8; 4] {
        let mut return_value = [0; 4];
        LittleEndian::write_f32(&mut return_value, value.into());
        return_value
    }

    fn i8_to_f32(&self, value: i8) -> [u8; 4] {
        let mut return_value = [0; 4];
        LittleEndian::write_f32(&mut return_value, value.into());
        return_value
    }

    fn f32_to_f32(&self, value: f32) -> [u8; 4] {
        let mut return_value = [0; 4];
        LittleEndian::write_f32(&mut return_value, value);
        return_value
    }

    fn gain_value(&self, value: u16) -> [u8; 4] {
        let mut return_value = [0; 4];
        LittleEndian::write_u16(&mut return_value[2..], value);
        return_value
    }

    /*
    Gate attenuation is an interesting one, it's stored and represented as a percent,
    but implemented as a non-linear array, so we're going to implement this the same way
    the Windows client does.
     */
    fn gate_attenuation_from_percent(&self, value: u8) -> i8 {
        let index = value as f32 * 0.24;

        if value > 99 {
            return GATE_ATTENUATION[25];
        }

        GATE_ATTENUATION[index as usize]
    }

    pub fn get_common_keys(&self) -> HashSet<EffectKey> {
        let mut keys = HashSet::new();
        keys.insert(EffectKey::DeEsser);
        keys.insert(EffectKey::GateThreshold);
        keys.insert(EffectKey::GateAttack);
        keys.insert(EffectKey::GateRelease);
        keys.insert(EffectKey::GateAttenuation);
        keys.insert(EffectKey::CompressorThreshold);
        keys.insert(EffectKey::CompressorRatio);
        keys.insert(EffectKey::CompressorAttack);
        keys.insert(EffectKey::CompressorRelease);
        keys.insert(EffectKey::CompressorMakeUpGain);
        keys.insert(EffectKey::GateEnabled);
        keys.insert(EffectKey::BleepLevel);
        keys.insert(EffectKey::GateMode);
        keys.insert(EffectKey::DisableMic);

        // TODO: Are these common?
        keys.insert(EffectKey::Encoder1Enabled);
        keys.insert(EffectKey::Encoder2Enabled);
        keys.insert(EffectKey::Encoder3Enabled);
        keys.insert(EffectKey::Encoder4Enabled);

        keys.insert(EffectKey::RobotEnabled);
        keys.insert(EffectKey::HardTuneEnabled);
        keys.insert(EffectKey::MegaphoneEnabled);

        keys
    }

    pub fn get_full_keys(&self) -> HashSet<EffectKey> {
        let mut keys = HashSet::new();

        // Lets go mental, return everything that's not common..
        let common_effects = self.get_common_keys();

        for effect in EffectKey::iter() {
            if !common_effects.contains(&effect) {
                keys.insert(effect);
            }
        }

        keys
    }

    // These are specific Group Key sets, useful for applying a specific effect at once.
    pub fn get_reverb_keyset(&self) -> LinkedHashSet<EffectKey> {
        let mut set = LinkedHashSet::new();
        set.insert(EffectKey::ReverbAmount);
        set.insert(EffectKey::ReverbType);
        set.insert(EffectKey::ReverbDecay);
        set.insert(EffectKey::ReverbPredelay);
        set.insert(EffectKey::ReverbDiffuse);
        set.insert(EffectKey::ReverbLowColor);
        set.insert(EffectKey::ReverbHighColor);
        set.insert(EffectKey::ReverbHighFactor);
        set.insert(EffectKey::ReverbModSpeed);
        set.insert(EffectKey::ReverbModDepth);
        set.insert(EffectKey::ReverbEarlyLevel);
        set.insert(EffectKey::ReverbTailLevel);

        set
    }

    pub fn get_echo_keyset(&self) -> LinkedHashSet<EffectKey> {
        let mut set = LinkedHashSet::new();
        set.insert(EffectKey::EchoAmount);
        set.insert(EffectKey::EchoSource);
        set.insert(EffectKey::EchoDivL);
        set.insert(EffectKey::EchoDivR);
        set.insert(EffectKey::EchoFeedbackL);
        set.insert(EffectKey::EchoFeedbackR);
        set.insert(EffectKey::EchoXFBLtoR);
        set.insert(EffectKey::EchoXFBRtoL);
        set.insert(EffectKey::EchoFeedback);
        set.insert(EffectKey::EchoFilterStyle);
        set.insert(EffectKey::EchoTempo);
        set.insert(EffectKey::EchoDelayL);
        set.insert(EffectKey::EchoDelayR);

        set
    }

    pub fn get_pitch_keyset(&self) -> LinkedHashSet<EffectKey> {
        let mut set = LinkedHashSet::new();
        set.insert(EffectKey::PitchAmount);
        set.insert(EffectKey::PitchCharacter);
        set.insert(EffectKey::PitchThreshold);

        set
    }

    pub fn get_gender_keyset(&self) -> LinkedHashSet<EffectKey> {
        let mut set = LinkedHashSet::new();
        set.insert(EffectKey::GenderAmount);

        set
    }

    pub fn get_megaphone_keyset(&self) -> LinkedHashSet<EffectKey> {
        let mut set = LinkedHashSet::new();
        set.insert(EffectKey::MegaphoneEnabled);
        set.insert(EffectKey::MegaphoneStyle);
        set.insert(EffectKey::MegaphoneAmount);
        set.insert(EffectKey::MegaphoneHP);
        set.insert(EffectKey::MegaphoneLP);
        set.insert(EffectKey::MegaphonePreGain);
        set.insert(EffectKey::MegaphonePostGain);
        set.insert(EffectKey::MegaphoneDistType);
        set.insert(EffectKey::MegaphonePresenceGain);
        set.insert(EffectKey::MegaphonePresenceFC);
        set.insert(EffectKey::MegaphonePresenceBW);
        set.insert(EffectKey::MegaphoneBeatboxEnable);
        set.insert(EffectKey::MegaphoneFilterControl);
        set.insert(EffectKey::MegaphoneFilter);
        set.insert(EffectKey::MegaphoneDrivePotGainCompMid);
        set.insert(EffectKey::MegaphoneDrivePotGainCompMax);

        set
    }

    pub fn get_robot_keyset(&self) -> LinkedHashSet<EffectKey> {
        let mut set = LinkedHashSet::new();
        set.insert(EffectKey::RobotEnabled);
        set.insert(EffectKey::RobotPulseWidth);
        set.insert(EffectKey::RobotWaveform);
        set.insert(EffectKey::RobotThreshold);
        set.insert(EffectKey::RobotDryMix);
        set.insert(EffectKey::RobotLowFreq);
        set.insert(EffectKey::RobotLowGain);
        set.insert(EffectKey::RobotLowWidth);
        set.insert(EffectKey::RobotMidFreq);
        set.insert(EffectKey::RobotMidGain);
        set.insert(EffectKey::RobotMidWidth);
        set.insert(EffectKey::RobotHiFreq);
        set.insert(EffectKey::RobotHiGain);
        set.insert(EffectKey::RobotHiWidth);
        set.insert(EffectKey::RobotStyle);

        set
    }

    pub fn get_hardtune_keyset(&self) -> LinkedHashSet<EffectKey> {
        let mut set = LinkedHashSet::new();
        set.insert(EffectKey::HardTuneEnabled);
        set.insert(EffectKey::HardTuneKeySource);
        set.insert(EffectKey::HardTuneAmount);
        set.insert(EffectKey::HardTuneWindow);
        set.insert(EffectKey::HardTuneRate);
        set.insert(EffectKey::HardTuneScale);
        set.insert(EffectKey::HardTunePitchAmount);

        set
    }

    pub fn get_deesser(&self) -> u8 {
        self.profile.deess()
    }
}
