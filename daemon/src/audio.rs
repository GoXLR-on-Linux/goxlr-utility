use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use log::{debug, error, info, warn};

#[derive(Debug)]
pub struct AudioHandler {
    script_path: PathBuf,
    output_device: String,
    input_device: Option<String>,
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
            let proj_dirs = ProjectDirs::from(
                "org",
                "GoXLR-on-Linux",
                "GoXLR-Utility")
                .context("Couldn't find project directories")?;

            script_path = proj_dirs.data_dir().join("goxlr-audio.sh");
        }
        debug!("Checking For {}", script_path.to_string_lossy());


        // This is temporary, just grab the script in the dev directory.
        if !script_path.exists() {
            error!("Unable to locate GoXLR Audio Script, Sampler Disabled.");
            return Err(anyhow!("Unable to locate GoXLR Audio Script, Sampler Disabled."));
        }
        debug!("Found GoXLR Audio script in {}", script_path.to_string_lossy());

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
            return Err(anyhow!("Unable to find sample output device, Sampler Disabled."));
        }
        let output_device = String::from_utf8(sampler_out.stdout)?;
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
            input_device
        })
    }

    // This will go away eventually, we should always keep track of stuff :p
    pub fn play_and_forget(&self, file: String) {
        // Simply spawn the script and ignore the result..
        let _command = Command::new(self.get_script())
            .arg("play-file")
            .arg(&self.output_device)
            .arg(file)
            .spawn();
    }

    fn get_script(&self) -> &str {
        self.script_path.to_str().unwrap()
    }
}