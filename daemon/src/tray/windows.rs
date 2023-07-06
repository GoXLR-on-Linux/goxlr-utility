use std::mem;
use std::ptr::null_mut;
use std::sync::atomic::Ordering;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{bail, Result};
use goxlr_ipc::PathTypes;
use log::{debug, warn};
use tokio::sync::mpsc::Sender;
use win_win::{WindowBuilder, WindowClass, WindowProc};
use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{DWORD, FALSE, HINSTANCE, LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HBRUSH, HICON, HMENU, HWND, POINT};
use winapi::um::shellapi::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW};
use winapi::um::synchapi::WaitForSingleObject;
use winapi::um::winnt::HANDLE;
use winapi::um::winuser::{
    AppendMenuW, CreateIcon, DestroyWindow, DispatchMessageW, PeekMessageW, TranslateMessage,
    MENUINFO, MF_POPUP, MF_SEPARATOR, MF_STRING, MIM_APPLYTOSUBMENUS, MIM_STYLE, MNS_NOTIFYBYPOS,
    PM_REMOVE,
};
use winapi::um::{shellapi, winuser};

use crate::events::EventTriggers::{Activate, Open};
use crate::events::{DaemonState, EventTriggers};
use crate::platform::to_wide;
use crate::tray::get_icon_from_global;

pub fn handle_tray(state: DaemonState, tx: Sender<EventTriggers>) -> Result<()> {
    debug!("Spawning Windows Tray..");

    // We jump this into another thread because on Windows it's tricky to shut down the window
    // properly, so it'll close when main() terminates.
    create_window(state, tx)?;
    Ok(())
}
fn create_window(state: DaemonState, tx: Sender<EventTriggers>) -> Result<()> {
    // To save some headaches, this is *ALL* unsafe!
    unsafe {
        // Load up the icon..
        let icon = load_icon()?;

        // Use win_win to setup our Window..
        let win_class = WindowClass::builder("goxlr-utility").build().unwrap();

        let sub = winuser::CreatePopupMenu();
        AppendMenuW(sub, MF_STRING, 10, to_wide("Profiles").as_ptr());
        AppendMenuW(sub, MF_STRING, 11, to_wide("Mic Profiles").as_ptr());
        AppendMenuW(sub, MF_SEPARATOR, 12, null_mut());
        AppendMenuW(sub, MF_STRING, 13, to_wide("Presets").as_ptr());
        AppendMenuW(sub, MF_STRING, 14, to_wide("Samples").as_ptr());
        AppendMenuW(sub, MF_STRING, 15, to_wide("Icons").as_ptr());

        // Create the Main Menu..
        let hmenu = winuser::CreatePopupMenu();
        AppendMenuW(hmenu, MF_STRING, 0, to_wide("Configure GoXLR").as_ptr());
        AppendMenuW(hmenu, MF_SEPARATOR, 1, null_mut());
        AppendMenuW(hmenu, MF_POPUP, sub as usize, to_wide("Open Path").as_ptr());
        AppendMenuW(hmenu, MF_SEPARATOR, 3, null_mut());
        AppendMenuW(hmenu, MF_STRING, 4, to_wide("Quit").as_ptr());

        let window_proc = GoXLRWindowProc::new(state.clone(), tx, hmenu);
        let hwnd = WindowBuilder::new(window_proc, &win_class).build();

        // Create the notification tray item..
        let mut tray_item = get_notification_struct(&hwnd);
        tray_item.szTip = tooltip("GoXLR Utility");
        tray_item.hIcon = icon;
        tray_item.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
        tray_item.uCallbackMessage = winuser::WM_USER + 1;

        if shellapi::Shell_NotifyIconW(NIM_ADD, &mut tray_item as *mut NOTIFYICONDATAW) == 0 {
            bail!("Unable to Create Tray Icon");
        }

        // Run our Main loop..
        run_loop(hwnd, state);

        // If we get here, the loop is done, remove our tray icon.
        if shellapi::Shell_NotifyIconW(NIM_DELETE, &mut tray_item as *mut NOTIFYICONDATAW) == 0 {
            bail!("Unable to remove Tray Icon!");
        }
    }

    Ok(())
}

fn run_loop(msg_window: HWND, state: DaemonState) {
    // Because we need to keep track of other things here, we're going to use PeekMessageW rather
    // than GetMessageW, then use WaitForSingleObject with a timeout to keep the loop looping.
    unsafe {
        loop {
            let mut msg = mem::MaybeUninit::uninit();
            if PeekMessageW(msg.as_mut_ptr(), msg_window, 0, 0, PM_REMOVE) != FALSE {
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

            // Wait either 20ms, or until a message comes in for the next pass.
            WaitForSingleObject(msg_window as HANDLE, 20);
        }
    }
}

fn load_icon() -> Result<HICON> {
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

    fn create_menu(&self) {
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
                // Window has spawned, Create our Menu :)
                debug!("Window Spawned, creating menu..");
                self.create_menu();
            }
            winuser::WM_QUERYENDSESSION => {
                // This will fall through and default to 'True'
                debug!("Query End Session Received..");
            }

            // Menu Related Commands..
            winuser::WM_MENUCOMMAND => unsafe {
                if lparam as HMENU == self.menu {
                    debug!("Top Menu?");
                }
                let menu_id = winuser::GetMenuItemID(lparam as HMENU, wparam as i32) as i32;
                let _ = match menu_id {
                    // Main Menu
                    0 => self.global_tx.try_send(EventTriggers::OpenUi),
                    4 => self.global_tx.try_send(EventTriggers::Stop),

                    // Open Paths Menu
                    10 => self.global_tx.try_send(Open(PathTypes::Profiles)),
                    11 => self.global_tx.try_send(Open(PathTypes::MicProfiles)),
                    13 => self.global_tx.try_send(Open(PathTypes::Presets)),
                    14 => self.global_tx.try_send(Open(PathTypes::Samples)),
                    15 => self.global_tx.try_send(Open(PathTypes::Icons)),

                    // Anything Else(?!)
                    id => {
                        warn!("Unexpected Menu Item: {}", id);
                        Ok(())
                    }
                };
            },

            0x401 => {
                if lparam as UINT == winuser::WM_LBUTTONUP
                    || lparam as UINT == winuser::WM_RBUTTONUP
                {
                    let mut point = POINT { x: 0, y: 0 };
                    unsafe {
                        if winuser::GetCursorPos(&mut point as *mut POINT) == 0 {
                            return Some(1);
                        }
                        if lparam as UINT == winuser::WM_LBUTTONUP {
                            let _ = self.global_tx.try_send(Activate);
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

            // Shutdown Handlers..
            winuser::WM_ENDSESSION => {
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
            _event => {}
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
    NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as DWORD,
        hWnd: *hwnd,
        uID: 1,
        uFlags: 0,
        uCallbackMessage: 0,
        hIcon: 0 as HICON,
        szTip: [0; 128],
        dwState: 0,
        dwStateMask: 0,
        szInfo: [0; 256],
        u: Default::default(),
        szInfoTitle: [0; 64],
        dwInfoFlags: 0,
        guidItem: GUID::default(),
        hBalloonIcon: 0 as HICON,
    }
}
