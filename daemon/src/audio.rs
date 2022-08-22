use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use goxlr_profile_loader::SampleButtons;
use log::{debug, error};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Debug)]
pub struct AudioHandler {
    script_path: PathBuf,
    output_device: String,
    _input_device: Option<String>,

    active_streams: HashMap<SampleButtons, Child>,
}

impl AudioHandler {
    pub fn new() -> Result<Self> {
        debug!("Preparing Audio Handler..");
        debug!("Looking for audio execution script..");

        // We're going to look for the file 'goxlr-audio.sh' in the following places:
        // 1) /usr/share/goxlr
        // -- This allows distros to provide their own scripts
        // 2) ~/.local/share/goxlr-on-linux/
        // -- We'll write an embedded script there if it's not present in 1

        // TODO: include_bytes!(from build), and write to 2 if not present.
        let mut script_path = Path::new("/usr/share/goxlr/goxlr-audio.sh").to_path_buf();
        debug!("Checking For {}", script_path.to_string_lossy());

        if !script_path.exists() {
            let proj_dirs = ProjectDirs::from("org", "GoXLR-on-Linux", "GoXLR-Utility")
                .context("Couldn't find project directories")?;

            script_path = proj_dirs.data_dir().join("goxlr-audio.sh");
        }
        debug!("Checking For {}", script_path.to_string_lossy());

        // This is temporary, just grab the script in the dev directory.
        if !script_path.exists() {
            error!("Unable to locate GoXLR Audio Script, Sampler Disabled.");
            return Err(anyhow!(
                "Unable to locate GoXLR Audio Script, Sampler Disabled."
            ));
        }
        debug!(
            "Found GoXLR Audio script in {}",
            script_path.to_string_lossy()
        );

        let script = script_path.to_str().expect("Unable to get the Script Path");

        debug!("Attempting to find Sample Output Device..");
        let sampler_out = Command::new(script)
            .arg("get-output-device")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Unable to Execute Script");

        if !sampler_out.status.success() {
            error!("{}", String::from_utf8(sampler_out.stderr)?);
            error!("Unable to find sample output device, Sampler Disabled.");
            return Err(anyhow!(
                "Unable to find sample output device, Sampler Disabled."
            ));
        }

        debug!("Sampler Output Device Check Returned OK");

        let output_device: String;
        if let Ok(device) = String::from_utf8(sampler_out.stdout) {
            output_device = device;
        } else {
            error!("Unable to parse String from UTF-8");
            return Err(anyhow!("Unable to parse UTF-8"));
        }

        //let output_device = String::from_utf8(sampler_out.stdout)?;
        let output_device = output_device.trim().to_string();
        debug!("Found output Device: {}", output_device);

        // Now get the recorder
        debug!("Attempting to find Sampler Input Device..");
        let sampler_in = Command::new(script)
            .arg("get-input-device")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Unable to Execute Script");

        let mut input_device = None;
        if !sampler_in.status.success() {
            error!("{}", String::from_utf8(sampler_in.stderr)?);
            error!("Unable to find sample capture device, Sample recording disabled.");
        } else {
            let found = String::from_utf8(sampler_in.stdout)?;
            input_device = Some(found.trim().to_string());
            debug!("Found input Device: {}", found.trim());
        }

        Ok(Self {
            script_path,
            output_device,
            _input_device: input_device,

            active_streams: HashMap::new(),
        })
    }

    pub fn check_playing(&mut self) {
        let map = &mut self.active_streams;
        let mut to_remove = Vec::new();

        for (key, value) in &mut *map {
            match value.try_wait() {
                Ok(Some(status)) => {
                    debug!("PID {} has terminated: {}", value.id(), status);
                    to_remove.push(*key);
                }
                Ok(None) => {
                    // Process hasn't terminated yet..
                }
                Err(e) => {
                    error!("Error checking wait {}", e)
                }
            }
        }

        for key in to_remove.iter() {
            map.remove(key);
        }
    }

    pub fn is_sample_playing(&self, button: SampleButtons) -> bool {
        self.active_streams.contains_key(&button)
    }

    pub fn play_for_button(&mut self, button: SampleButtons, file: String) -> Result<()> {
        let command = Command::new(self.get_script())
            .arg("play-file")
            .arg(&self.output_device)
            .arg(file)
            .spawn()
            .expect("Unable to run script");

        self.active_streams.insert(button, command);
        Ok(())
    }

    fn get_script(&self) -> &str {
        self.script_path.to_str().unwrap()
    }
}
