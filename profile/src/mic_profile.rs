use crate::microphone::compressor::Compressor;
use crate::microphone::equalizer::Equalizer;
use crate::microphone::equalizer_mini::EqualizerMini;
use crate::microphone::gate::Gate;
use crate::microphone::mic_setup::MicSetup;
use crate::microphone::ui_setup::UiSetup;
use crate::profile::wrap_start_event;
use anyhow::{anyhow, bail, Result};
use log::debug;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::os::raw::c_float;
use std::path::Path;

#[derive(Debug)]
pub struct MicProfileSettings {
    equalizer: Equalizer,
    equalizer_mini: EqualizerMini,
    compressor: Compressor,
    gate: Gate,
    deess: u8,
    bleep_level: i8,
    gate_mode: u8,
    comp_select: u8,
    mic_setup: MicSetup,
    ui_setup: UiSetup,
}

impl MicProfileSettings {
    pub fn load<R: Read>(read: R) -> Result<Self> {
        let buf_reader = BufReader::new(read);
        let mut reader = Reader::from_reader(buf_reader);

        //let parser = EventReader::new(read);

        let mut equalizer = Equalizer::new();
        let mut equalizer_mini = EqualizerMini::new();
        let mut compressor = Compressor::new();
        let mut gate = Gate::new();
        let mut deess = 0;
        let mut bleep_level = -20;
        let mut gate_mode = 2;
        let mut comp_select = 1;
        let mut mic_setup = MicSetup::new();
        let mut ui_setup = UiSetup::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Empty(ref e)) => {
                    let (name, attributes) = wrap_start_event(e)?;
                    if name == "dspTreeMicProfile" {
                        // Ok, this is an incredibly large tag, with many settings (30 or so), so
                        // we split it into 3 separate elements.
                        equalizer.parse_equaliser(&attributes)?;
                        equalizer_mini.parse_equaliser(&attributes)?;
                        compressor.parse_compressor(&attributes)?;
                        gate.parse_gate(&attributes)?;

                        // Before we're done here, there's a single attribute that doesn't fit into
                        // any of the above categories, find it and handle it here..
                        for attr in &attributes {
                            if attr.name == "MIC_DEESS_AMOUNT" {
                                deess = attr.value.parse::<c_float>()? as u8;
                                continue;
                            }
                            if attr.name == "BLEEP_LEVEL" {
                                bleep_level = attr.value.parse::<c_float>()? as i8;
                                continue;
                            }
                            if attr.name == "MIC_COMP_SELECT" {
                                comp_select = attr.value.parse::<c_float>()? as u8;
                                continue;
                            }
                            if attr.name == "MIC_GATE_MODE" {
                                gate_mode = attr.value.parse::<c_float>()? as u8;
                                continue;
                            }
                        }

                        continue;
                    }

                    if name == "setupTreeMicProfile" {
                        mic_setup.parse_config(&attributes)?;
                        continue;
                    }

                    if name == "micProfileUIMicProfile" {
                        ui_setup.parse_ui(&attributes)?;
                        continue;
                    }

                    if name == "MicProfileTree" {
                        continue;
                    }

                    println!("Unhandled Tag: {name}");
                }

                Ok(Event::Eof) => {
                    break;
                }

                // Event::Start/End only occurs for the top level micProfileTree, which has no
                // attributes, so we don't need to worry about it :)
                Ok(_) => {}
                Err(e) => {
                    bail!("Error Parsing Profile: {}", e);
                }
            }
        }

        Ok(Self {
            equalizer,
            equalizer_mini,
            compressor,
            gate,
            deess,
            bleep_level,
            gate_mode,
            comp_select,
            mic_setup,
            ui_setup,
        })
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        debug!("Saving File: {}", &path.as_ref().to_string_lossy());

        let out_file = File::create(path)?;
        self.write_to(out_file)?;

        Ok(())
    }

    pub fn write_to<W: Write>(&self, sink: W) -> Result<()> {
        let mut writer = Writer::new_with_indent(sink, u8::try_from('\t')?, 1);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
        writer.write_event(Event::Start(BytesStart::new("MicProfileTree")))?;

        // First, we need to write the EQ, Compressor and Gate..
        let mut attributes: HashMap<String, String> = HashMap::default();

        // The mini and main can both have configs in the same file.
        self.equalizer.write_equaliser(&mut attributes);
        self.equalizer_mini.write_equaliser(&mut attributes);
        self.compressor.write_compressor(&mut attributes);
        self.gate.write_gate(&mut attributes);
        attributes.insert("MIC_DEESS_AMOUNT".to_string(), format!("{}", self.deess));
        attributes.insert(
            "MIC_COMP_SELECT".to_string(),
            format!("{}", self.comp_select),
        );
        attributes.insert("BLEEP_LEVEL".to_string(), format!("{}", self.bleep_level));
        attributes.insert("MIC_GATE_MODE".to_string(), format!("{}", self.gate_mode));

        let mut elem = BytesStart::new("dspTreeMicProfile");
        for (key, value) in &attributes {
            elem.push_attribute((key.as_str(), value.as_str()));
        }
        writer.write_event(Event::Empty(elem))?;

        self.mic_setup.write_config(&mut writer)?;
        self.ui_setup.write_ui(&mut writer)?;

        writer.write_event(Event::End(BytesEnd::new("MicProfileTree")))?;
        Ok(())
    }

    pub fn setup_mut(&mut self) -> &mut MicSetup {
        &mut self.mic_setup
    }
    pub fn setup(&self) -> &MicSetup {
        &self.mic_setup
    }

    pub fn ui_setup(&self) -> &UiSetup {
        &self.ui_setup
    }
    pub fn ui_setup_mut(&mut self) -> &mut UiSetup {
        &mut self.ui_setup
    }

    pub fn gate(&self) -> &Gate {
        &self.gate
    }
    pub fn gate_mut(&mut self) -> &mut Gate {
        &mut self.gate
    }
    pub fn compressor(&self) -> &Compressor {
        &self.compressor
    }
    pub fn compressor_mut(&mut self) -> &mut Compressor {
        &mut self.compressor
    }
    pub fn equalizer(&self) -> &Equalizer {
        &self.equalizer
    }
    pub fn equalizer_mut(&mut self) -> &mut Equalizer {
        &mut self.equalizer
    }

    pub fn equalizer_mini(&self) -> &EqualizerMini {
        &self.equalizer_mini
    }
    pub fn equalizer_mini_mut(&mut self) -> &mut EqualizerMini {
        &mut self.equalizer_mini
    }
    pub fn deess(&self) -> u8 {
        self.deess
    }
    pub fn set_deess(&mut self, deess: u8) -> Result<()> {
        if deess > 100 {
            return Err(anyhow!("De-Ess value must be a percentage"));
        }
        self.deess = deess;
        Ok(())
    }

    pub fn bleep_level(&self) -> i8 {
        self.bleep_level
    }
    pub fn set_bleep_level(&mut self, bleep_level: i8) -> Result<()> {
        if !(-36..=0).contains(&bleep_level) {
            return Err(anyhow!("Bleep level should be between -34 and 0"));
        }
        self.bleep_level = bleep_level;
        Ok(())
    }

    pub fn gate_mode(&self) -> u8 {
        self.gate_mode
    }
    pub fn set_gate_mode(&mut self, gate_mode: u8) {
        self.gate_mode = gate_mode;
    }

    pub fn comp_select(&self) -> u8 {
        self.comp_select
    }
    pub fn set_comp_select(&mut self, comp_select: u8) {
        self.comp_select = comp_select;
    }
}
