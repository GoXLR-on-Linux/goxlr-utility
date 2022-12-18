use anyhow::Result;
use std::sync::atomic::Ordering;
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn;

use crate::events::EventTriggers;
use crate::{DaemonState, ICON};
use futures::executor::block_on;
use log::debug;
use tao::event::Event;
use tao::event::TrayEvent::LeftClick;
use tao::menu::MenuItem::Separator;
use tao::menu::{ContextMenu, MenuItemAttributes, MenuType};
use tao::system_tray::SystemTrayBuilder;
use tao::TrayId;
use tokio::sync::mpsc;

use goxlr_ipc::PathTypes::{Icons, MicProfiles, Presets, Profiles, Samples};
#[cfg(target_os = "macos")]
use tao::platform::macos::{
    ActivationPolicy, EventLoopExtMacOS, SystemTrayBuilderExtMacOS, SystemTrayExtMacOS,
};

pub fn handle_tray(state: DaemonState, tx: mpsc::Sender<EventTriggers>) -> Result<()> {
    let mut event_loop = EventLoop::new();

    #[cfg(target_os = "macos")]
    {
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop.set_activate_ignoring_other_apps(true);
    }

    let tray_id = TrayId::new("goxlr-utility-tray");
    let icon = load_icon();

    // Create the 'Paths' Submenu
    let mut sub_menu = ContextMenu::new();
    let profiles = sub_menu.add_item(MenuItemAttributes::new("Profiles"));
    let mic_profiles = sub_menu.add_item(MenuItemAttributes::new("Mic Profiles"));
    sub_menu.add_native_item(Separator);
    let presets = sub_menu.add_item(MenuItemAttributes::new("Presets"));
    let samples = sub_menu.add_item(MenuItemAttributes::new("Samples"));
    let icons = sub_menu.add_item(MenuItemAttributes::new("Icons"));

    let mut tray_menu = ContextMenu::new();
    let configure = tray_menu.add_item(MenuItemAttributes::new("Configure GoXLR"));
    tray_menu.add_native_item(Separator);
    tray_menu.add_submenu("Open Path", true, sub_menu);
    tray_menu.add_native_item(Separator);
    let quit = tray_menu.add_item(MenuItemAttributes::new("Quit"));

    let system_tray = SystemTrayBuilder::new(icon.clone(), Some(tray_menu))
        .with_id(tray_id)
        .with_tooltip("GoXLR Utility")
        .build(&event_loop)?;

    // So the problem is, on certain OSs, the Event Loop handler *HAS* to be handled on
    // the main thread. So this is a blocking call. We'll keep an eye out for the shutdown
    // handle being changed, so we can exit gracefully when Ctrl+C is hit.
    event_loop.run_return(move |event, _event_loop, control_flow| {
        // We set this to poll, so we can monitor both the menu, and tray icon..
        //*control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));
        *control_flow = ControlFlow::Poll;

        match event {
            Event::MenuEvent {
                menu_id,
                origin: MenuType::ContextMenu,
                ..
            } => {
                if menu_id == quit.clone().id() {
                    let _ = block_on(tx.send(EventTriggers::Stop));
                }

                if menu_id == configure.clone().id() {
                    let _ = block_on(tx.send(EventTriggers::OpenUi));
                }

                if menu_id == profiles.clone().id() {
                    let _ = block_on(tx.send(EventTriggers::Open(Profiles)));
                }

                if menu_id == mic_profiles.clone().id() {
                    let _ = block_on(tx.send(EventTriggers::Open(MicProfiles)));
                }

                if menu_id == presets.clone().id() {
                    let _ = block_on(tx.send(EventTriggers::Open(Presets)));
                }

                if menu_id == samples.clone().id() {
                    let _ = block_on(tx.send(EventTriggers::Open(Samples)));
                }

                if menu_id == icons.clone().id() {
                    let _ = block_on(tx.send(EventTriggers::Open(Icons)));
                }
            }
            Event::TrayEvent { event, .. } => {
                // Left click on Mac opens the menu, so we don't want to trigger this.
                if event == LeftClick && !cfg!(macos) {
                    let _ = block_on(tx.send(EventTriggers::OpenUi));
                }
            }
            _ => {}
        }

        if state.shutdown_blocking.load(Ordering::Relaxed) {
            debug!("Shutting down Window Event Handler..");
            *control_flow = ControlFlow::Exit;
        }
    });

    drop(system_tray);
    Ok(())
}

fn load_icon() -> tao::system_tray::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(ICON)
            .expect("Failed to load Icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tao::system_tray::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .expect("Failed to load Icon")
}