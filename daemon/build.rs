use clap::CommandFactory;
use clap_complete::{generate_to, Shell};
use std::env;
use std::fs::File;
use std::io::Error;
use std::path::Path;

#[cfg(target_os = "windows")]
use windres::Build;

include!("src/cli.rs");

fn main() -> Result<(), Error> {
    #[cfg(target_os = "windows")]
    {
        Build::new().compile("resources/goxlr-daemon.rc").unwrap();
    }

    let outdir = match env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => outdir,
    };

    let mut app = Cli::command();
    for shell in Shell::value_variants() {
        let _ = generate_to(*shell, &mut app, "goxlr-daemon", &outdir)?;
    }

    let stamp_path = Path::new(&outdir).join("daemon-stamp");
    if let Err(err) = File::create(&stamp_path) {
        panic!("failed to write {}: {}", stamp_path.display(), err);
    }

    Ok(())
}
