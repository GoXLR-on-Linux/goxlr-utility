use std::io::Error;

#[cfg(target_os = "windows")]
use windres::Build;

fn main() -> Result<(), Error> {
    #[cfg(target_os = "windows")]
    {
        Build::new()
            .compile("./resources/goxlr-launcher.rc")
            .unwrap();
    }

    Ok(())
}
