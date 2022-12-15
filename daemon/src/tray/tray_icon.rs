use anyhow::Result;
use tray_icon::menu::{menu_event_receiver, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{tray_event_receiver, ClickEvent, TrayIconBuilder};
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::platform::run_return::EventLoopExtRunReturn;

#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

use crate::events::EventTriggers;
use crate::{DaemonState, ICON};
use futures::executor::block_on;
use log::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub fn handle_tray(
    shutdown: Arc<AtomicBool>,
    tx: mpsc::Sender<EventTriggers>,
    state: DaemonState,
) -> Result<()> {
    // Create the Event Loop..
    let mut builder = EventLoopBuilder::new();

    #[cfg(target_os = "macos")]
    builder.with_activation_policy(ActivationPolicy::Prohibited);

    let mut event_loop = builder.build();

    loop {
        debug!("Starting Loop..");
        // If we're at the top of the loop, and shutdown has been requested, stop.
        if state.shutdown_blocking.load(Ordering::Relaxed) {
            break;
        }
        if !state.show_tray.load(Ordering::Relaxed) {
            debug!("Icon Hidden..");

            // We're not showing the icon, so sleep and see if that changes..
            thread::sleep(Duration::from_millis(50));
            continue;
        }

        let tray_menu = Menu::new();
        let configure = MenuItem::new("Configure GoXLR", true, None);
        let quit = MenuItem::new("Quit", true, None);
        tray_menu.append_items(&[&configure, &PredefinedMenuItem::separator(), &quit]);

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("GoXLR Utility")
            .with_icon(load_icon())
            .build()?;

        let tray_channel = tray_event_receiver();
        let menu_channel = menu_event_receiver();

        let sender = tx.clone();
        let shutdown_monitor = state.shutdown_blocking.clone();
        let show_icon = state.show_tray.clone();

        // So the problem is, on certain OSs, the Event Loop handler *HAS* to be handled on
        // the main thread. So this is a blocking call. We'll keep an eye out for the shutdown
        // handle being changed, so we can exit gracefully when Ctrl+C is hit.

        event_loop.run_return(move |_event, _, control_flow| {
            // We set this to poll, so we can monitor both the menu, and tray icon..
            if *control_flow != ControlFlow::Exit {
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));
            }

            if let Ok(event) = menu_channel.try_recv() {
                if event.id == configure.id() {
                    let _ = block_on(sender.send(EventTriggers::OpenUi));
                }

                if event.id == quit.id() {
                    let _ = block_on(sender.send(EventTriggers::Stop));
                    *control_flow = ControlFlow::Exit;
                }
            }

            if let Ok(event) = tray_channel.try_recv() {
                // Did the User left click on the icon?
                if event.event == ClickEvent::Left {
                    // Is this windows?
                    if cfg!(target_os = "windows") {
                        let _ = block_on(sender.send(EventTriggers::OpenUi));
                    }
                }
            }

            if !show_icon.load(Ordering::Relaxed) {
                // We've been instructed to hide the icon, so breakout the event loop.
                debug!("Icon no longer visible, exit Event Loop..");
                *control_flow = ControlFlow::Exit;
            }

            if shutdown_monitor.load(Ordering::Relaxed) {
                debug!("Shutting down Window Event Handler..");
                *control_flow = ControlFlow::Exit;
            }
        });

        // When we get here we're done with the event listener. We need to drop the tray icon
        // to ensure any 'background' cleanup is done.
        drop(tray_icon);
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
    }
    Ok(())
}

fn load_icon() -> tray_icon::icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(ICON)
            .expect("Failed to load Icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .expect("Failed to load Icon")
}
