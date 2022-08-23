use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use goxlr_profile_loader::SampleButtons;
use log::{debug, error};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct AudioHandler {
    script_path: PathBuf,
    output_device: Option<String>,
    _input_device: Option<String>,

    last_device_check: Instant,

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
            return Err(anyhow!(
                "Unable to locate GoXLR Audio Script, Sampler Disabled."
            ));
        }
        debug!(
            "Found GoXLR Audio script in {}",
            script_path.to_string_lossy()
        );

        let mut handler = Self {
            script_path,
            output_device: None,
            _input_device: None,

            last_device_check: Instant::now(),
            active_streams: HashMap::new(),
        };

        handler.output_device = handler.find_device("get-output-device")?;
        handler._input_device = handler.find_device("get-input-device")?;

        Ok(handler)
    }

    fn find_device(&self, arg: &str) -> Result<Option<String>> {
        debug!("Attempting to Find Device..");
        let command = Command::new(&self.script_path)
            .arg(arg)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        let mut input_device = None;
        if command.status.success() {
            let found = String::from_utf8(command.stdout)?;
            input_device = Some(found.trim().to_string());
            debug!("Found Device: {}", found.trim());
        } else {
            error!("Script Says: {}", String::from_utf8(command.stderr)?.trim());
            error!("Unable to find sample device, will retry in 10 seconds");
        }

        Ok(input_device)
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
        if self.output_device.is_none()
            && (self.last_device_check + Duration::from_secs(5)) < Instant::now()
        {
            // Perform a re-check, to see if the devices have become available..
            self.output_device = self.find_device("get-output-device")?;
            self.last_device_check = Instant::now();
        }

        if let Some(output_device) = &self.output_device {
            let command = Command::new(self.get_script())
                .arg("play-file")
                .arg(output_device)
                .arg(file)
                .spawn()?;
            self.active_streams.insert(button, command);
        } else {
            return Err(anyhow!("Unable to play Sample, Output device not found"));
        }

        Ok(())
    }

    fn get_script(&self) -> &str {
        self.script_path.to_str().unwrap()
    }
}
