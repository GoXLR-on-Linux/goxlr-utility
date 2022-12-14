use anyhow::Result;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod tray_icon;

pub fn handle_tray(blocking_shutdown: Arc<AtomicBool>) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::handle_tray(blocking_shutdown)
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        #[cfg(target_os = "macos")]
        {
            use cocoa::appkit::NSApp;
            use cocoa::appkit::NSApplication;
            use cocoa::appkit::NSApplicationActivationPolicy;

            // Before we spawn the tray, we need to initialise the app (this doesn't appear to
            // be done by tray-icon)
            unsafe {
                let app = NSApp();
                app.setActivationPolicy_(
                    NSApplicationActivationPolicy::NSApplicationActivationPolicyAccessory,
                );
            }
        }
        tray_icon::handle_tray(blocking_shutdown)
    }

    // For all other platforms, don't attempt to spawn a Tray Icon
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        // For now, don't spawn a tray icon.
        Ok(())
    }
}
