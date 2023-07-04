use anyhow::Result;
use std::sync::Arc;

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn;

use crate::events::EventTriggers;
use crate::{DaemonState, ICON};
use cfg_if::cfg_if;
use log::debug;
use std::time::{Duration, Instant};
use tao::event::Event;
use tao::event::TrayEvent::LeftClick;
use tao::menu::MenuItem::Separator;
use tao::menu::{ContextMenu, MenuItemAttributes, MenuType};
use tao::system_tray::SystemTrayBuilder;
use tao::TrayId;
use tokio::sync::mpsc::Sender;

use goxlr_ipc::PathTypes::{Icons, MicProfiles, Presets, Profiles, Samples};
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};

cfg_if! {
    if #[cfg(windows)] {
        use std::thread;
        use win_win::{WindowBuilder, WindowClass, WindowProc};
        use winapi::um::winuser::{ShowWindow, SW_HIDE};
        use std::ptr::null_mut;
    }
}

pub fn handle_tray(state: DaemonState, tx: Sender<EventTriggers>) -> Result<()> {
    let mut event_loop = EventLoop::new();

    #[cfg(target_os = "macos")]
    {
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
        event_loop.set_activate_ignoring_other_apps(true);
    }

    let ready = Arc::new(AtomicBool::new(false));
    let ready_inner = ready.clone();
    #[cfg(windows)]
    {
        let win_state = state.clone();
        let win_global = tx.clone();
        thread::spawn(move || create_window(win_state, win_global, ready_inner));
    }

    #[cfg(not(windows))]
    {
        ready.store(true, Ordering::Relaxed);
    }

    // Ok, we need to loop here until the main window is ready..
    while !ready.load(Ordering::Relaxed) {
        sleep(Duration::from_millis(5));
    }
    debug!("Launching Tray..");

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

    let system_tray = SystemTrayBuilder::new(icon, Some(tray_menu))
        .with_id(tray_id)
        .with_tooltip("GoXLR Utility")
        .build(&event_loop)?;

    // So the problem is, on certain OSs, the Event Loop handler *HAS* to be handled on
    // the main thread. So this is a blocking call. We'll keep an eye out for the shutdown
    // handle being changed, so we can exit gracefully when Ctrl+C is hit.
    event_loop.run_return(move |event, _event_loop, control_flow| {
        // We set this to poll, so we can monitor both the menu, and tray icon..
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(50));

        match event {
            Event::MenuEvent {
                menu_id,
                origin: MenuType::ContextMenu,
                ..
            } => {
                if menu_id == quit.clone().id() {
                    let _ = tx.try_send(EventTriggers::Stop);
                }

                if menu_id == configure.clone().id() {
                    let _ = tx.try_send(EventTriggers::OpenUi);
                }

                if menu_id == profiles.clone().id() {
                    let _ = tx.try_send(EventTriggers::Open(Profiles));
                }

                if menu_id == mic_profiles.clone().id() {
                    let _ = tx.try_send(EventTriggers::Open(MicProfiles));
                }

                if menu_id == presets.clone().id() {
                    let _ = tx.try_send(EventTriggers::Open(Presets));
                }

                if menu_id == samples.clone().id() {
                    let _ = tx.try_send(EventTriggers::Open(Samples));
                }

                if menu_id == icons.clone().id() {
                    let _ = tx.try_send(EventTriggers::Open(Icons));
                }
            }
            Event::TrayEvent { event, .. } => {
                // Left click on Mac opens the menu, so we don't want to trigger this.
                if event == LeftClick && cfg!(not(target_os = "macos")) {
                    let _ = tx.try_send(EventTriggers::Activate);
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

#[cfg(windows)]
fn create_window(state: DaemonState, tx: Sender<EventTriggers>, ready: Arc<AtomicBool>) {
    let win_class = WindowClass::builder("goxlr-utility").build().unwrap();
    let window_proc = GoXLRWindowProc::new(state, tx, ready);
    let hwnd = WindowBuilder::new(window_proc, &win_class).build();

    unsafe {
        ShowWindow(hwnd, SW_HIDE);
        win_win::runloop(null_mut());
    }
}

#[cfg(windows)]
struct GoXLRWindowProc {
    state: DaemonState,
    global_tx: Sender<EventTriggers>,
    ready: Arc<AtomicBool>,
}

#[cfg(windows)]
impl GoXLRWindowProc {
    pub fn new(state: DaemonState, tx: Sender<EventTriggers>, ready: Arc<AtomicBool>) -> Self {
        Self {
            state,
            global_tx: tx,
            ready,
        }
    }
}

#[cfg(windows)]
impl WindowProc for GoXLRWindowProc {
    fn window_proc(
        &self,
        _hwnd: winapi::shared::windef::HWND,
        msg: winapi::shared::minwindef::UINT,
        _wparam: winapi::shared::minwindef::WPARAM,
        _lparam: winapi::shared::minwindef::LPARAM,
    ) -> Option<winapi::shared::minwindef::LRESULT> {
        match msg {
            winapi::um::winuser::WM_CREATE => {
                // Window has spawned, allow other code to continue.
                debug!("Window Spawned, resuming startup..");
                self.ready.store(true, Ordering::Relaxed);
            }
            winapi::um::winuser::WM_QUERYENDSESSION => {
                // This will fall through and default to 'True'
                debug!("Query End Session Received..");
            }
            winapi::um::winuser::WM_ENDSESSION => {
                debug!("Received WM_ENDSESSION from Windows");

                // Ok, Windows has prompted an session closure here, we need to make sure the
                // daemon shuts down correctly..
                debug!("Attempting Shutdown..");
                let _ = self.global_tx.try_send(EventTriggers::Stop);

                // Now wait for the daemon to actually stop..
                loop {
                    if self.state.shutdown_blocking.load(Ordering::Relaxed) {
                        break;
                    } else {
                        debug!("Waiting..");
                        sleep(Duration::from_millis(100));
                    }
                }

                debug!("Shutdown Complete?");
            }
            _ => {}
        }
        None
    }
}
