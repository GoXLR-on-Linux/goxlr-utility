use std::ffi::c_void;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{bail, Result};
use lazy_static::lazy_static;
use log::{debug, error, warn};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;
use windows::core::imp::GetLastError;
use windows::core::w;
use windows::Win32::Foundation::{FALSE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Shutdown::{ShutdownBlockReasonCreate, ShutdownBlockReasonDestroy};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GetMessageW, GetWindowLongPtrW, RegisterClassW, RegisterWindowMessageW,
    SetMenuInfo, SetTimer, SetWindowLongPtrW, TranslateMessage, CREATESTRUCTW, CS_HREDRAW,
    CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, HICON, HMENU, MENUINFO, MF_POPUP, MF_SEPARATOR,
    MF_STRING, MIM_APPLYTOSUBMENUS, MIM_STYLE, MNS_NOTIFYBYPOS, WINDOW_EX_STYLE, WM_CREATE,
    WM_NCDESTROY, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

use goxlr_ipc::PathTypes;

use crate::events::EventTriggers::Open;
use crate::events::{DaemonState, EventTriggers};
use crate::tray::get_icon_from_global;

const EVENT_MESSAGE: u32 = WM_USER + 1;

lazy_static! {
    static ref RESPAWN: u32 = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
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
        debug!("Creating SubMenu");
        let sub = CreatePopupMenu()?;
        AppendMenuW(sub, MF_STRING, 10, w!("Profiles"))?;
        AppendMenuW(sub, MF_STRING, 11, w!("Mic Profiles"))?;
        AppendMenuW(sub, MF_SEPARATOR, 12, None)?;
        AppendMenuW(sub, MF_STRING, 13, w!("Presets"))?;
        AppendMenuW(sub, MF_STRING, 14, w!("Samples"))?;
        AppendMenuW(sub, MF_STRING, 15, w!("Icons"))?;
        AppendMenuW(sub, MF_SEPARATOR, 16, None)?;
        AppendMenuW(sub, MF_STRING, 17, w!("Logs"))?;

        // Create the Main Menu..
        debug!("Creating Main Menu..");
        let hmenu = CreatePopupMenu()?;
        AppendMenuW(hmenu, MF_STRING, 0, w!("Configure GoXLR"))?;
        AppendMenuW(hmenu, MF_SEPARATOR, 1, None)?;
        AppendMenuW(hmenu, MF_POPUP, sub.0 as usize, w!("Open Path"))?;
        AppendMenuW(hmenu, MF_SEPARATOR, 3, None)?;
        AppendMenuW(hmenu, MF_STRING, 4, w!("Quit"))?;

        debug!("Generating Window Proc");
        let window_proc = GoXLRWindowProc::new(state.clone(), tx, hmenu);
        let wrapped_proc: Rc<Box<dyn WindowProc>> = Rc::new(Box::new(window_proc));

        debug!("Getting HWND");
        let hwnd = create_hwnd(wrapped_proc)?;

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
                let _ = DestroyWindow(msg_window);
                break;
            }

            // This will trigger a return of GetMessageW in theory..
            SetTimer(msg_window, 120, 20, None);
        }
    }
    debug!("Primary Loop Ended");
}

fn create_hwnd(proc: Rc<Box<dyn WindowProc>>) -> Result<HWND> {
    let h_instance: HINSTANCE = unsafe { GetModuleHandleW(None) }?.into();
    let lp_sz_class_name = w!("GoXLR Utility");
    let lp_sz_window_name = w!("GoXLR Utility");

    // Create our Window Class..
    let window_class = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(raw_window_proc),
        hInstance: h_instance,
        lpszClassName: lp_sz_class_name,
        ..Default::default()
    };

    // Register it..
    if unsafe { RegisterClassW(&window_class) } == 0 {
        bail!(unsafe { GetLastError() });
    }

    // Now attempt to create our HWND...
    let window_pointer = Rc::into_raw(proc) as *const c_void;
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            window_class.lpszClassName,
            lp_sz_window_name,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            window_class.hInstance,
            Some(window_pointer),
        )
    };

    // Attempt to Create the Tray Icon..
    if hwnd == HWND(0) {
        bail!(unsafe { GetLastError() });
    }

    Ok(hwnd)
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
            HINSTANCE(0),
            width as i32,
            height as i32,
            1,
            32_u8,
            alpha_mask.as_ptr(),
            rgba.as_ptr(),
        )
    }?;
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

    fn create_tray(&self, hwnd: HWND) -> Option<windows::Win32::UI::Shell::NOTIFYICONDATAW> {
        if let Ok(icon) = load_icon() {
            debug!("Generating Tray Item");

            let mut tray_item = get_notification_struct(hwnd);
            tray_item.szTip = tooltip("GoXLR Utility");
            tray_item.hIcon = icon;
            tray_item.uFlags = windows::Win32::UI::Shell::NIF_MESSAGE
                | windows::Win32::UI::Shell::NIF_TIP
                | windows::Win32::UI::Shell::NIF_ICON;
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
        self.spawn_tray(hwnd, windows::Win32::UI::Shell::NIM_ADD);
    }

    fn destroy_icon(&self, hwnd: HWND) {
        if !self.state.show_tray.load(Ordering::Relaxed) {
            return;
        }
        debug!("Destroying Tray Icon");
        self.spawn_tray(hwnd, windows::Win32::UI::Shell::NIM_DELETE);
    }

    fn spawn_tray(&self, hwnd: HWND, action: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) {
        debug!("Creating Tray Handler");
        if let Some(mut tray) = self.create_tray(hwnd) {
            let tray = &mut tray as *mut windows::Win32::UI::Shell::NOTIFYICONDATAW;

            unsafe {
                debug!("Performing Tray Action");
                if windows::Win32::UI::Shell::Shell_NotifyIconW(action, tray) == FALSE {
                    error!("Unable to Load Tray Icon");
                }
            }
        }
    }

    fn create_menu(&self) {
        debug!("Creating Menu");
        let m = MENUINFO {
            cbSize: mem::size_of::<MENUINFO>() as u32,
            fMask: MIM_APPLYTOSUBMENUS | MIM_STYLE,
            dwStyle: MNS_NOTIFYBYPOS,
            cyMax: 0,
            hbrBack: HBRUSH::default(),
            dwContextHelpID: 0,
            dwMenuData: 0,
        };
        unsafe {
            debug!("Setting Menu Info");
            if SetMenuInfo(self.menu, &m as *const MENUINFO).is_err() {
                warn!("Error Setting Up Menu.");
            };
        }
    }
}

impl WindowProc for GoXLRWindowProc {
    fn window_proc(&self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        use windows::Win32::UI::WindowsAndMessaging::*;
        //debug!("{:?} - {:?} - {:?} - {:?}", hwnd, msg, wparam, lparam);

        match msg {
            WM_CREATE => {
                debug!("Window Created, Spawn icon and Menu");

                // Window has spawned, Create our Menu :)
                self.create_icon(hwnd);
                self.create_menu();
            }
            // Menu Related Commands..
            WM_MENUCOMMAND => unsafe {
                // We're going to grab the isize pointer to the menu, then pass that in.
                let hmenu = lparam.0 as *const isize as isize;
                let npos = wparam.0 as *const i32 as i32;

                let menu_id = GetMenuItemID(HMENU(hmenu), npos);
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
                let button = lparam.0 as *const u32 as u32;
                if button == WM_LBUTTONUP || button == WM_RBUTTONUP {
                    let mut point = POINT { x: 0, y: 0 };
                    unsafe {
                        if GetCursorPos(&mut point as *mut POINT).is_err() {
                            return Some(LRESULT(1));
                        }
                        if button == WM_LBUTTONUP {
                            let _ = self.global_tx.try_send(EventTriggers::Activate);
                            return None;
                        }
                        if button == WM_RBUTTONUP {
                            // The docs say if the window isn't foreground, the menu wont close!
                            SetForegroundWindow(hwnd);

                            // Create the menu at the coordinates of the mouse.
                            TrackPopupMenu(
                                self.menu,
                                TRACK_POPUP_MENU_FLAGS(0),
                                point.x,
                                point.y,
                                0,
                                hwnd,
                                None,
                            );
                        }
                    }
                }
            }

            WM_DESTROY => {
                debug!("Windows Destroyed, killing Tray Icon");
                self.destroy_icon(hwnd);
            }

            // Window Handler
            WM_CLOSE => {
                // If something tries to close this hidden window, it's a good bet that it wants
                // us to shutdown, start the shutdown, but don't close the Window.
                let _ = self.global_tx.try_send(EventTriggers::Stop);
                return Some(LRESULT(1));
            }

            // // Shutdown Handlers..
            WM_QUERYENDSESSION => {
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
                    let _ = ShutdownBlockReasonCreate(hwnd, w!("Running Shutdown.."));
                }

                debug!("Attempting Shutdown..");
                let _ = self.global_tx.try_send(EventTriggers::Stop);

                // Now wait for the daemon to actually stop..
                loop {
                    if self.state.shutdown_blocking.load(Ordering::Relaxed) {
                        unsafe {
                            let _ = ShutdownBlockReasonDestroy(hwnd);
                        }
                        break;
                    } else {
                        debug!("Waiting..");
                        sleep(Duration::from_millis(100));
                    }
                }
            }
            WM_ENDSESSION => {
                debug!("Received WM_ENDSESSION from Windows, Doing nothing..");
            }
            WM_POWERBROADCAST => {
                debug!("Received POWER Broadcast from Windows");
                let param = wparam.0 as *const u32 as u32;
                if param == PBT_APMSUSPEND {
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
                if param == PBT_APMRESUMESUSPEND {
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
fn get_notification_struct(hwnd: HWND) -> windows::Win32::UI::Shell::NOTIFYICONDATAW {
    windows::Win32::UI::Shell::NOTIFYICONDATAW {
        hWnd: hwnd,
        uID: 1,
        uFlags: windows::Win32::UI::Shell::NOTIFY_ICON_DATA_FLAGS(0),
        uCallbackMessage: 0,
        ..Default::default()
    }
}

unsafe extern "system" fn raw_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_CREATE {
        let create_struct = &*(lparam.0 as *const CREATESTRUCTW);
        let window_pointer = create_struct.lpCreateParams;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_pointer as isize);
    }

    let window_pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Box<dyn WindowProc>;
    let result = {
        if window_pointer.is_null() {
            None
        } else {
            let reference = Rc::from_raw(window_pointer);
            mem::forget(reference.clone());
            (*window_pointer).window_proc(hwnd, msg, wparam, lparam)
        }
    };

    if msg == WM_NCDESTROY && !window_pointer.is_null() {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        drop(Rc::from_raw(window_pointer));
    }
    result.unwrap_or_else(|| DefWindowProcW(hwnd, msg, wparam, lparam))
}

// This is our trait, so we can build a struct and call into it..
trait WindowProc {
    fn window_proc(&self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT>;
}
