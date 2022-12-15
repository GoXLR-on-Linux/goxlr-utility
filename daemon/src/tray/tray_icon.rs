use anyhow::Result;
use tray_icon::menu::{menu_event_receiver, Menu, MenuItem};
use tray_icon::{tray_event_receiver, ClickEvent, TrayIconBuilder};
use winit::event_loop::{ControlFlow, EventLoopBuilder};
use winit::platform::run_return::EventLoopExtRunReturn;

#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

use crate::tray::event_manager::Message;
use crate::ICON;
use futures::executor::block_on;
use log::debug;
use notify::Event;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

pub fn handle_tray(shutdown: Arc<AtomicBool>, tx: mpsc::Sender<Message>) -> Result<()> {
    let tray_menu = Menu::new();
    let configure = MenuItem::new("Configure GoXLR", true, None);
    tray_menu.append_items(&[&configure]);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("GoXLR Utility")
        .with_icon(load_icon())
        .build()?;

    let tray_channel = tray_event_receiver();
    let menu_channel = menu_event_receiver();

    let mut builder = EventLoopBuilder::new();

    #[cfg(target_os = "macos")]
    builder.with_activation_policy(ActivationPolicy::Prohibited);

    // So the problem is, on certain OSs, the Event Loop handler *HAS* to be handled on
    // the main thread. So this is a blocking call. We'll keep an eye out for the shutdown
    // handle being changed, so we can exit gracefully when Ctrl+C is hit.
    let mut event_loop = builder.build();
    event_loop.run_return(move |_event, _, control_flow| {
        // We set this to poll, so we can monitor both the menu, and tray icon..
        if *control_flow != ControlFlow::Exit {
            *control_flow = ControlFlow::Poll;
        }

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == configure.id() {
                debug!("Configure Button Pressed");
                let _ = block_on(tx.send(Message::Open));
            }
            debug!("{:?}", event);
        }

        if let Ok(event) = tray_channel.try_recv() {
            // Did the User left click on the icon?
            if event.event == ClickEvent::Left {
                // Is this windows?
                if cfg!(target_os = "windows") {
                    let _ = block_on(tx.send(Message::Open));
                }
            }

            debug!("{:?}", event);
        }

        if shutdown.load(Ordering::Relaxed) {
            debug!("Shutting down Window Event Handler..");
            *control_flow = ControlFlow::Exit;
        }
    });

    // When we get here we're done with the event listener. We need to drop the tray icon
    // to ensure any 'background' cleanup is done.
    drop(tray_icon);
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
