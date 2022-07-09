use clap::{ArgEnum, IntoApp};
use clap_complete::{generate_to, Shell};
use std::env;
use std::io::Error;

include!("src/cli.rs");

fn main() -> Result<(), Error> {
    let outdir = match env::var_os("OUT_DIR") {
        None => return Ok(()),
        Some(outdir) => outdir,
    };

    let mut app = Cli::into_app();
    for shell in Shell::value_variants() {
        let path = generate_to(*shell, &mut app, "goxlr-daemon", &outdir)?;

        println!(
            "cargo:warning={} completion file is generated: {:?}",
            shell, path
        );
    }

    Ok(())
}
