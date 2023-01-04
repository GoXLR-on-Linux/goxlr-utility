#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{bail, Result};
use clap::Parser;
use std::fs;

use include_dir::{include_dir, Dir};
use std::path::PathBuf;

const PROFILES: Dir = include_dir!("./defaults/resources/profiles");
const MIC_PROFILES: Dir = include_dir!("./defaults/resources/mic-profiles");
const PRESETS: Dir = include_dir!("./defaults/resources/presets");
const ICONS: Dir = include_dir!("./defaults/resources/icons");

fn main() -> Result<()> {
    let args: Cli = Cli::parse();

    // Check if the provided path exists, and is a directory..
    if !args.file_path.exists() {
        bail!("Provided Path does not exist");
    }
    if !args.file_path.is_dir() {
        bail!("Provided Path is not a directory");
    }

    let files = match args.file_type {
        Type::Profiles => PROFILES,
        Type::MicProfiles => MIC_PROFILES,
        Type::Presets => PRESETS,
        Type::Icons => ICONS,
    };

    // Iterate through the embedded files..
    for file in files.files() {
        let file_path = args.file_path.join(file.path());

        if file_path.exists() {
            if !args.overwrite {
                continue;
            } else {
                fs::remove_file(&file_path)?;
            }
        }

        fs::write(&file_path, file.contents())?;
    }

    Ok(())
}

#[derive(Debug, Parser)]
struct Cli {
    /// The type of files to be extracted
    #[clap(value_enum)]
    file_type: Type,

    /// The Path to Extract the files to
    file_path: PathBuf,

    /// Whether to Overwrite existing files
    #[clap(long)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum Type {
    Profiles,
    MicProfiles,
    Presets,
    Icons,
}
