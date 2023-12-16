use std::mem;
use std::mem::zeroed;
use std::ptr::null_mut;
use std::sync::atomic::Ordering;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{bail, Result};
use lazy_static::lazy_static;
use log::{debug, error, warn};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use win_win::{WindowBuilder, WindowClass, WindowProc};
use winapi::shared::minwindef::{DWORD, FALSE, HINSTANCE, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HBRUSH, HICON, HMENU, HWND, POINT};
use winapi::um::shellapi::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use winapi::um::winuser::{
    AppendMenuW, CreateIcon, DestroyWindow, DispatchMessageW, GetMessageW, RegisterWindowMessageW,
    SetTimer, ShutdownBlockReasonCreate, ShutdownBlockReasonDestroy, TranslateMessage, MENUINFO,
    MF_POPUP, MF_SEPARATOR, MF_STRING, MIM_APPLYTOSUBMENUS, MIM_STYLE, MNS_NOTIFYBYPOS, WM_USER,
};
use winapi::um::{shellapi, winuser};

use goxlr_ipc::PathTypes;

use crate::events::EventTriggers::Open;
use crate::events::{DaemonState, EventTriggers};
use crate::platform::to_wide;
use crate::tray::get_icon_from_global;

const EVENT_MESSAGE: u32 = WM_USER + 1;

lazy_static! {
    static ref RESPAWN: UINT =
        unsafe { RegisterWindowMessageW(to_wide("TaskbarCreated").as_ptr()) };
}

pub fn handle_tray(state: DaemonState, tx: Sender<EventTriggers>) -> Result<()> {
    debug!("Spawning Windows Tray..");

    // We jump this into another thread because on Windows it's tricky to shut down the window
    // properly, so it'll close when main() terminates.
    create_window(state, tx)?;
    Ok(())
}
fn create_window(state: DaemonState, tx: Sender<EventTriggers>) -> Result<()> {
    // To save some headaches, this is *ALL* unsafe!
    debug!("Creating Window for Tray");
    unsafe {
        // Use win_win to setup our Window..
        let win_class = WindowClass::builder("goxlr-utility").build().unwrap();

        debug!("Creating SubMenu");
        let sub = winuser::CreatePopupMenu();
        AppendMenuW(sub, MF_STRING, 10, to_wide("Profiles").as_ptr());
        AppendMenuW(sub, MF_STRING, 11, to_wide("Mic Profiles").as_ptr());
        AppendMenuW(sub, MF_SEPARATOR, 12, null_mut());
        AppendMenuW(sub, MF_STRING, 13, to_wide("Presets").as_ptr());
        AppendMenuW(sub, MF_STRING, 14, to_wide("Samples").as_ptr());
        AppendMenuW(sub, MF_STRING, 15, to_wide("Icons").as_ptr());
        AppendMenuW(sub, MF_SEPARATOR, 16, null_mut());
        AppendMenuW(sub, MF_STRING, 17, to_wide("Logs").as_ptr());

        // Create the Main Menu..
        debug!("Creating Main Menu..");
        let hmenu = winuser::CreatePopupMenu();
        AppendMenuW(hmenu, MF_STRING, 0, to_wide("Configure GoXLR").as_ptr());
        AppendMenuW(hmenu, MF_SEPARATOR, 1, null_mut());
        AppendMenuW(hmenu, MF_POPUP, sub as usize, to_wide("Open Path").as_ptr());
        AppendMenuW(hmenu, MF_SEPARATOR, 3, null_mut());
        AppendMenuW(hmenu, MF_STRING, 4, to_wide("Quit").as_ptr());

        debug!("Generating Window Proc");
        let window_proc = GoXLRWindowProc::new(state.clone(), tx, hmenu);

        debug!("Getting HWND");
        let hwnd = WindowBuilder::new(window_proc, &win_class)
            .name("GoXLR Utility")
            .size(20, 20)
            .build();

        debug!("Beginning Tray Runtime Loop");
        run_loop(hwnd, state.clone());
    }

    Ok(())
}

fn run_loop(msg_window: HWND, state: DaemonState) {
    // Because we need to keep track of other things here, we're going to use PeekMessageW rather
    // than GetMessageW, then use WaitForSingleObject with a timeout to keep the loop looping.

    debug!("Running Main Window Loop");
    // Turns out, WaitForSingleObject doesn't work for window HWNDs..
    unsafe {
        // Send a message to the window to be be processed 20ms after we hit here..
        SetTimer(msg_window, 120, 20, None);
        loop {
            let mut msg = mem::MaybeUninit::uninit();
            if GetMessageW(msg.as_mut_ptr(), msg_window, 0, 0) != FALSE {
                let msg = msg.assume_init();

                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Check to see if we've reached Shutdown Stage 2..
            if state.shutdown_blocking.load(Ordering::Relaxed) {
                debug!("Shutdown Phase 2 active, destroy the window.");
                DestroyWindow(msg_window);
                break;
            }

            // This will trigger a return of GetMessageW in theory..
            SetTimer(msg_window, 120, 20, None);
        }
    }
    debug!("Primary Loop Ended");
}

fn load_icon() -> Result<HICON> {
    debug!("Loading Tray Icon");
    let (rgba, width, height) = get_icon_from_global();

    let count = rgba.len() / 4;
    let mut alpha_mask = Vec::with_capacity(count);
    for slice in rgba.chunks(4) {
        alpha_mask.push(slice[3].wrapping_sub(u8::MAX));
    }

    let icon = unsafe {
        CreateIcon(
            0 as HINSTANCE,
            width as i32,
            height as i32,
            1,
            32_u8,
            alpha_mask.as_ptr(),
            rgba.as_ptr(),
        )
    };
    if icon == null_mut() as HICON {
        bail!("Unable to Load Icon");
    }
    Ok(icon)
}

#[cfg(windows)]
struct GoXLRWindowProc {
    state: DaemonState,
    global_tx: Sender<EventTriggers>,
    menu: HMENU,
}

impl GoXLRWindowProc {
    pub fn new(state: DaemonState, tx: Sender<EventTriggers>, menu: HMENU) -> Self {
        Self {
            state,
            global_tx: tx,
            menu,
        }
    }

    fn create_tray(&self, hwnd: HWND) -> Option<NOTIFYICONDATAW> {
        if let Ok(icon) = load_icon() {
            debug!("Generating Tray Item");
            let mut tray_item = get_notification_struct(&hwnd);
            tray_item.szTip = tooltip("GoXLR Utility");
            tray_item.hIcon = icon;
            tray_item.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
            tray_item.uCallbackMessage = EVENT_MESSAGE;

            return Some(tray_item);
        }
        None
    }

    fn create_icon(&self, hwnd: HWND) {
        if !self.state.show_tray.load(Ordering::Relaxed) {
            debug!("Tray Disabled, doing nothing.");
            return;
        }

        debug!("Calling Tray Spawner");
        self.spawn_tray(hwnd, NIM_ADD);
    }

    fn destroy_icon(&self, hwnd: HWND) {
        if !self.state.show_tray.load(Ordering::Relaxed) {
            return;
        }
        debug!("Destroying Tray Icon");
        self.spawn_tray(hwnd, NIM_DELETE);
    }

    fn spawn_tray(&self, hwnd: HWND, action: DWORD) {
        debug!("Creating Tray Handler");
        if let Some(mut tray) = self.create_tray(hwnd) {
            let tray = &mut tray as *mut NOTIFYICONDATAW;

            unsafe {
                debug!("Performing Tray Action");
                if shellapi::Shell_NotifyIconW(action, tray) == 0 {
                    error!("Unable to Load Tray Icon");
                }
            }
        }
    }

    fn create_menu(&self) {
        debug!("Creating Menu");
        let m = MENUINFO {
            cbSize: mem::size_of::<MENUINFO>() as DWORD,
            fMask: MIM_APPLYTOSUBMENUS | MIM_STYLE,
            dwStyle: MNS_NOTIFYBYPOS,
            cyMax: 0,
            hbrBack: 0 as HBRUSH,
            dwContextHelpID: 0,
            dwMenuData: 0,
        };
        unsafe {
            debug!("Setting Menu Info");
            if winuser::SetMenuInfo(self.menu, &m as *const MENUINFO) == 0 {
                warn!("Error Setting Up Menu.");
            }
        }
    }
}

impl WindowProc for GoXLRWindowProc {
    fn window_proc(
        &self,
        hwnd: HWND,
        msg: UINT,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match msg {
            winuser::WM_CREATE => {
                debug!("Window Created, Spawn icon and Menu");

                // Window has spawned, Create our Menu :)
                self.create_icon(hwnd);
                self.create_menu();
            }
            // Menu Related Commands..
            winuser::WM_MENUCOMMAND => unsafe {
                let menu_id = winuser::GetMenuItemID(lparam as HMENU, wparam as i32) as i32;
                let _ = match menu_id {
                    // Main Menu
                    0 => self.global_tx.try_send(EventTriggers::Activate),
                    4 => self.global_tx.try_send(EventTriggers::Stop),

                    // Open Paths Menu
                    10 => self.global_tx.try_send(Open(PathTypes::Profiles)),
                    11 => self.global_tx.try_send(Open(PathTypes::MicProfiles)),
                    13 => self.global_tx.try_send(Open(PathTypes::Presets)),
                    14 => self.global_tx.try_send(Open(PathTypes::Samples)),
                    15 => self.global_tx.try_send(Open(PathTypes::Icons)),
                    17 => self.global_tx.try_send(Open(PathTypes::Logs)),

                    // Anything Else(?!)
                    id => {
                        warn!("Unexpected Menu Item: {}", id);
                        Ok(())
                    }
                };
            },

            EVENT_MESSAGE => {
                if lparam as UINT == winuser::WM_LBUTTONUP
                    || lparam as UINT == winuser::WM_RBUTTONUP
                {
                    let mut point = POINT { x: 0, y: 0 };
                    unsafe {
                        if winuser::GetCursorPos(&mut point as *mut POINT) == 0 {
                            return Some(1);
                        }
                        if lparam as UINT == winuser::WM_LBUTTONUP {
                            let _ = self.global_tx.try_send(EventTriggers::Activate);
                            return None;
                        }
                        if lparam as UINT == winuser::WM_RBUTTONUP {
                            // The docs say if the window isn't foreground, the menu wont close!
                            winuser::SetForegroundWindow(hwnd);

                            // Create the menu at the coordinates of the mouse.
                            winuser::TrackPopupMenu(
                                self.menu,
                                0,
                                point.x,
                                point.y,
                                (winuser::TPM_BOTTOMALIGN | winuser::TPM_LEFTALIGN) as i32,
                                hwnd,
                                null_mut(),
                            );
                        }
                    }
                }
            }

            winuser::WM_DESTROY => {
                debug!("Windows Destroyed, killing Tray Icon");
                self.destroy_icon(hwnd);
            }

            // Window Handler
            winuser::WM_CLOSE => {
                // If something tries to close this hidden window, it's a good bet that it wants
                // us to shutdown, start the shutdown, but don't close the Window.
                let _ = self.global_tx.try_send(EventTriggers::Stop);
                return Some(1);
            }

            // // Shutdown Handlers..
            winuser::WM_QUERYENDSESSION => {
                debug!("Received WM_QUERYENDSESSION from Windows, Shutting Down..");
                /*
                 Ref: https://learn.microsoft.com/en-us/windows/win32/shutdown/wm-queryendsession

                 Ok, long comment, according the docs:
                 "When an application returns TRUE for this message, it receives the WM_ENDSESSION
                  message, regardless of how the other applications respond to the
                  WM_QUERYENDSESSION message."

                  The problem we run into, is that the TTS service spawns an invisible window to
                  handle media playback, and if it receives the WM_ENDSESSION message and calls
                  DestroyWindow, Windows will assume the entire Utility is done and kill the
                  process. This prevents the WM_ENDSESSION message from reaching us here preventing
                  us from correctly handling the shutdown.

                  Consequently, we're forced to try and get ahead of it and handle our shutdown
                  behaviours in the 'wrong' place, but we're at least guaranteed to be handled.
                */

                unsafe {
                    ShutdownBlockReasonCreate(hwnd, to_wide("Running Shutdown").as_ptr());
                }

                debug!("Attempting Shutdown..");
                let _ = self.global_tx.try_send(EventTriggers::Stop);

                // Now wait for the daemon to actually stop..
                loop {
                    if self.state.shutdown_blocking.load(Ordering::Relaxed) {
                        unsafe {
                            ShutdownBlockReasonDestroy(hwnd);
                        }
                        break;
                    } else {
                        debug!("Waiting..");
                        sleep(Duration::from_millis(100));
                    }
                }
            }
            winuser::WM_ENDSESSION => {
                debug!("Received WM_ENDSESSION from Windows, Doing nothing..");
            }
            winuser::WM_POWERBROADCAST => {
                debug!("Received POWER Broadcast from Windows");

                if wparam == winuser::PBT_APMSUSPEND {
                    debug!("Suspend Requested by Windows, Handling..");
                    let (tx, mut rx) = oneshot::channel();

                    // Give a maximum of 1 second for a response..
                    let milli_wait = 5;
                    let max_wait = 1000 / milli_wait;
                    let mut count = 0;

                    // Only hold on the receiver if the send was successful..
                    if self.global_tx.try_send(EventTriggers::Sleep(tx)).is_ok() {
                        debug!("Awaiting Sleep Response..");
                        while rx.try_recv().is_err() {
                            sleep(Duration::from_millis(milli_wait));
                            count += 1;
                            if count > max_wait {
                                debug!("Timeout Exceeded, bailing.");
                                break;
                            }
                        }
                        debug!("Task Completed, allowing Windows to Sleep");
                    }
                }
                if wparam == winuser::PBT_APMRESUMESUSPEND {
                    debug!("Wake Signal Received..");
                    let (tx, _rx) = oneshot::channel();

                    // We're awake again, we don't need to care about the response here.
                    let _ = self.global_tx.try_send(EventTriggers::Wake(tx));
                }
            }
            _ => {
                if msg == *RESPAWN {
                    debug!("Icon respawn requested: {}", msg);
                    self.create_icon(hwnd);
                }
            }
        }
        None
    }
}

// Random Needed Windows Stuff..
fn tooltip(msg: &str) -> [u16; 128] {
    let mut array = [0; 128];

    let message = msg.as_bytes();
    for i in 0..msg.len() {
        array[i] = message[i] as u16;
    }
    array
}
fn get_notification_struct(hwnd: &HWND) -> NOTIFYICONDATAW {
    let mut icon: NOTIFYICONDATAW = unsafe { zeroed() };
    icon.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as DWORD;
    icon.hWnd = *hwnd;
    icon.uID = 1;
    icon.uFlags = 0;
    icon.uCallbackMessage = 0;

    icon
}
