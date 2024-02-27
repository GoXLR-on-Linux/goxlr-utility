pub mod autostart;
pub mod sleep;

pub fn display_error(message: String) {
    use std::process::Command;
    // We have two choices here, kdialog, or zenity. We'll try both.
    if let Err(e) = Command::new("kdialog")
        .arg("--title")
        .arg("GoXLR Utility")
        .arg("--error")
        .arg(message.clone())
        .output()
    {
        println!("Error Running kdialog: {}, falling back to zenity..", e);
        let _ = Command::new("zenity")
            .arg("--title")
            .arg("GoXLR Utility")
            .arg("--error")
            .arg("--text")
            .arg(message)
            .output();
    }
}
