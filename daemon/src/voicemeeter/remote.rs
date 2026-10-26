//! Voicemeeter Remote API access.
//!
//! The Remote API is not thread safe (IsParametersDirty must be called from a single thread), so
//! everything Voicemeeter related lives on a dedicated OS thread, talking to the async worker via
//! channels.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::mpsc as std_mpsc;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum RemoteStatus {
    /// Not a Windows build, Voicemeeter can't exist here.
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    Unsupported,

    /// Unable to load VoicemeeterRemote64.dll
    NotInstalled(String),

    /// The DLL is loaded, but Voicemeeter isn't running
    NotRunning,

    Connected,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VmChannel {
    pub label: String,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VmInfo {
    /// 1 = Voicemeeter, 2 = Banana, 3 = Potato
    pub kind: i32,
    pub version: String,
    pub strips: Vec<VmChannel>,
    pub buses: Vec<VmChannel>,
}

pub enum VmRequest {
    /// Replace the list of parameters to poll
    Watch(Vec<String>),
    Set(String, f32),
    Shutdown,
}

pub enum VmEvent {
    Status(RemoteStatus, Option<VmInfo>),

    /// Full snapshot of the watched parameter values (only sent when something changed)
    Values(HashMap<String, f32>),
}

pub fn spawn_remote(events: UnboundedSender<VmEvent>) -> std_mpsc::Sender<VmRequest> {
    let (tx, rx) = std_mpsc::channel();
    std::thread::Builder::new()
        .name("voicemeeter-remote".into())
        .spawn(move || imp::run(rx, events))
        .expect("Unable to spawn Voicemeeter thread");
    tx
}

/// Physical / total strip and bus counts for a Voicemeeter type
fn layout(kind: i32) -> ((usize, usize), (usize, usize)) {
    match kind {
        1 => ((2, 3), (1, 2)),
        2 => ((3, 5), (3, 5)),
        _ => ((5, 8), (5, 8)),
    }
}

/// Splits `MacroButton[3].State` into (3, bitmode)
fn parse_macro_button(param: &str) -> Option<(i32, i32)> {
    let rest = param.strip_prefix("MacroButton[")?;
    let (index, suffix) = rest.split_once("].")?;
    let index = index.trim().parse().ok()?;
    let mode = match suffix.to_ascii_lowercase().as_str() {
        "state" => 0,
        "stateonly" => 2,
        "trigger" => 3,
        _ => return None,
    };
    Some((index, mode))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use anyhow::{Result, anyhow};
    use log::{debug, info, warn};
    use std::ffi::{CString, c_char};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::core::{HSTRING, PCSTR};
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    const POLL_INTERVAL: Duration = Duration::from_millis(20);
    const RETRY_INTERVAL: Duration = Duration::from_secs(10);
    const INFO_INTERVAL: Duration = Duration::from_secs(2);

    const UNINSTALL_KEY: &str = "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\VB:Voicemeeter {17359A74-1236-5467}";
    const DEFAULT_DIR: &str = "C:\\Program Files (x86)\\VB\\Voicemeeter";
    const DLL_NAME: &str = "VoicemeeterRemote64.dll";

    type FnVoid = unsafe extern "system" fn() -> i32;
    type FnLongPtr = unsafe extern "system" fn(*mut i32) -> i32;
    type FnGetFloat = unsafe extern "system" fn(*const c_char, *mut f32) -> i32;
    type FnGetString = unsafe extern "system" fn(*const c_char, *mut u16) -> i32;
    type FnSetFloat = unsafe extern "system" fn(*const c_char, f32) -> i32;
    type FnMacroGet = unsafe extern "system" fn(i32, *mut f32, i32) -> i32;
    type FnMacroSet = unsafe extern "system" fn(i32, f32, i32) -> i32;

    struct Api {
        login: FnVoid,
        logout: FnVoid,
        get_type: FnLongPtr,
        get_version: FnLongPtr,
        is_dirty: FnVoid,
        get_float: FnGetFloat,
        get_string: FnGetString,
        set_float: FnSetFloat,
        macro_is_dirty: FnVoid,
        macro_get: FnMacroGet,
        macro_set: FnMacroSet,
    }

    fn dll_path() -> PathBuf {
        if let Some(path) = std::env::var_os("VOICEMEETER_REMOTE_DLL") {
            return PathBuf::from(path);
        }

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let key = UNINSTALL_KEY;
        let alt = key.replace("WOW6432Node\\", "");
        for key in [key, alt.as_str()] {
            if let Ok(key) = hklm.open_subkey(key)
                && let Ok(uninstall) = key.get_value::<String, _>("UninstallString")
            {
                let uninstall = PathBuf::from(uninstall.trim_matches('"'));
                if let Some(dir) = uninstall.parent() {
                    return dir.join(DLL_NAME);
                }
            }
        }
        PathBuf::from(DEFAULT_DIR).join(DLL_NAME)
    }

    impl Api {
        fn load() -> Result<Self> {
            let path = dll_path();
            debug!("Loading Voicemeeter Remote from {:?}", path);

            // The DLL is intentionally never freed, it lives for as long as the daemon does.
            let module = unsafe { LoadLibraryW(&HSTRING::from(path.as_os_str())) }
                .map_err(|e| anyhow!("{}: {}", path.display(), e))?;

            // Signatures from VoicemeeterRemote.h
            unsafe {
                Ok(Self {
                    login: proc(module, "VBVMR_Login\0")?,
                    logout: proc(module, "VBVMR_Logout\0")?,
                    get_type: proc(module, "VBVMR_GetVoicemeeterType\0")?,
                    get_version: proc(module, "VBVMR_GetVoicemeeterVersion\0")?,
                    is_dirty: proc(module, "VBVMR_IsParametersDirty\0")?,
                    get_float: proc(module, "VBVMR_GetParameterFloat\0")?,
                    get_string: proc(module, "VBVMR_GetParameterStringW\0")?,
                    set_float: proc(module, "VBVMR_SetParameterFloat\0")?,
                    macro_is_dirty: proc(module, "VBVMR_MacroButton_IsDirty\0")?,
                    macro_get: proc(module, "VBVMR_MacroButton_GetStatus\0")?,
                    macro_set: proc(module, "VBVMR_MacroButton_SetStatus\0")?,
                })
            }
        }

        fn get(&self, param: &str) -> Option<f32> {
            let mut value = 0.0f32;
            let result = if let Some((index, mode)) = parse_macro_button(param) {
                unsafe { (self.macro_get)(index, &mut value, mode) }
            } else {
                let name = CString::new(param).ok()?;
                unsafe { (self.get_float)(name.as_ptr(), &mut value) }
            };
            (result == 0).then_some(value)
        }

        fn set(&self, param: &str, value: f32) {
            let result = if let Some((index, mode)) = parse_macro_button(param) {
                unsafe { (self.macro_set)(index, value, mode) }
            } else if let Ok(name) = CString::new(param) {
                unsafe { (self.set_float)(name.as_ptr(), value) }
            } else {
                -1
            };
            if result != 0 {
                warn!(
                    "Voicemeeter: Unable to set {} to {} ({})",
                    param, value, result
                );
            }
        }

        fn get_string(&self, param: &str) -> Option<String> {
            let name = CString::new(param).ok()?;
            // Wide version, the ANSI one returns labels in the system code page
            let mut buffer = [0u16; 512];
            let result = unsafe { (self.get_string)(name.as_ptr(), buffer.as_mut_ptr()) };
            if result != 0 {
                return None;
            }
            let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
            Some(String::from_utf16_lossy(&buffer[..length]))
        }

        fn kind(&self) -> Option<i32> {
            let mut kind = 0;
            let result = unsafe { (self.get_type)(&mut kind) };
            (result == 0).then_some(kind)
        }

        fn info(&self, kind: i32) -> VmInfo {
            let mut version = 0;
            unsafe { (self.get_version)(&mut version) };
            let v = version as u32;
            let version = format!(
                "{}.{}.{}.{}",
                v >> 24,
                (v >> 16) & 0xFF,
                (v >> 8) & 0xFF,
                v & 0xFF
            );

            let ((strip_phys, strips), (bus_phys, buses)) = layout(kind);
            let channels = |prefix: &str, physical: usize, total: usize| {
                (0..total)
                    .map(|i| VmChannel {
                        label: self
                            .get_string(&format!("{prefix}[{i}].Label"))
                            .unwrap_or_default(),
                        is_virtual: i >= physical,
                    })
                    .collect()
            };

            VmInfo {
                kind,
                version,
                strips: channels("Strip", strip_phys, strips),
                buses: channels("Bus", bus_phys, buses),
            }
        }
    }

    /// Looks up an export and casts it to the function pointer type `T`, which must match the
    /// real signature.
    unsafe fn proc<T: Copy>(module: HMODULE, name: &'static str) -> Result<T> {
        let address = unsafe { GetProcAddress(module, PCSTR(name.as_ptr())) }
            .ok_or_else(|| anyhow!("Missing {} in {}", name.trim_end_matches('\0'), DLL_NAME))?;
        Ok(
            unsafe {
                std::mem::transmute_copy::<unsafe extern "system" fn() -> isize, T>(&address)
            },
        )
    }

    fn set_status(
        status: &mut Option<RemoteStatus>,
        events: &UnboundedSender<VmEvent>,
        new: RemoteStatus,
        info: Option<VmInfo>,
    ) {
        if status.as_ref() != Some(&new) {
            info!("Voicemeeter: {:?}", new);
            *status = Some(new.clone());
            let _ = events.send(VmEvent::Status(new, info));
        }
    }

    pub fn run(rx: std_mpsc::Receiver<VmRequest>, events: UnboundedSender<VmEvent>) {
        let mut api: Option<Api> = None;
        let mut logged_in = false;
        let mut next_attempt = Instant::now();

        let mut status: Option<RemoteStatus> = None;
        let mut info: Option<VmInfo> = None;
        let mut info_checked = Instant::now();

        let mut watched: Vec<String> = vec![];
        let mut values: HashMap<String, f32> = HashMap::new();
        let mut force_read = true;

        loop {
            // Handle requests, the timeout doubles as our poll interval
            let mut request = rx.recv_timeout(POLL_INTERVAL);
            loop {
                match request {
                    Ok(VmRequest::Watch(list)) => {
                        watched = list;
                        force_read = true;
                    }
                    Ok(VmRequest::Set(param, value)) => {
                        if let Some(api) = &api
                            && status == Some(RemoteStatus::Connected)
                        {
                            api.set(&param, value);
                        }
                    }
                    Ok(VmRequest::Shutdown) | Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                        if let Some(api) = &api
                            && logged_in
                        {
                            debug!("Logging out of Voicemeeter");
                            unsafe { (api.logout)() };
                        }
                        return;
                    }
                    Err(std_mpsc::RecvTimeoutError::Timeout) => break,
                }
                request = rx.try_recv().map_err(|e| match e {
                    std_mpsc::TryRecvError::Empty => std_mpsc::RecvTimeoutError::Timeout,
                    std_mpsc::TryRecvError::Disconnected => {
                        std_mpsc::RecvTimeoutError::Disconnected
                    }
                });
            }

            if api.is_none() || !logged_in {
                if Instant::now() < next_attempt {
                    continue;
                }
                next_attempt = Instant::now() + RETRY_INTERVAL;

                if api.is_none() {
                    match Api::load() {
                        Ok(loaded) => api = Some(loaded),
                        Err(e) => {
                            set_status(
                                &mut status,
                                &events,
                                RemoteStatus::NotInstalled(e.to_string()),
                                None,
                            );
                            continue;
                        }
                    }
                }

                if let Some(api) = &api {
                    // 0 = OK, 1 = OK but Voicemeeter isn't running, < 0 = error
                    let result = unsafe { (api.login)() };
                    if result < 0 {
                        set_status(
                            &mut status,
                            &events,
                            RemoteStatus::NotInstalled(format!("VBVMR_Login failed ({result})")),
                            None,
                        );
                        continue;
                    }
                    logged_in = true;
                }
            }

            let Some(api) = &api else { continue };
            match api.kind() {
                None => {
                    if status == Some(RemoteStatus::Connected) {
                        values.clear();
                        let _ = events.send(VmEvent::Values(values.clone()));
                    }
                    set_status(&mut status, &events, RemoteStatus::NotRunning, None);
                    continue;
                }
                Some(kind) => {
                    if status != Some(RemoteStatus::Connected) {
                        // Parameters need a dirty check before they're valid
                        unsafe { (api.is_dirty)() };
                        let new_info = api.info(kind);
                        info = Some(new_info.clone());
                        info_checked = Instant::now();
                        set_status(
                            &mut status,
                            &events,
                            RemoteStatus::Connected,
                            Some(new_info),
                        );
                        force_read = true;
                    }
                }
            }

            let dirty = unsafe { (api.is_dirty)() } > 0;
            let macro_dirty = unsafe { (api.macro_is_dirty)() } > 0;
            if dirty || macro_dirty || force_read {
                force_read = false;

                let new_values: HashMap<String, f32> = watched
                    .iter()
                    .filter_map(|param| api.get(param).map(|value| (param.clone(), value)))
                    .collect();
                if new_values != values {
                    values = new_values;
                    let _ = events.send(VmEvent::Values(values.clone()));
                }
            }

            // Labels may change while Voicemeeter is running, re-check them occasionally
            if dirty && info_checked.elapsed() > INFO_INTERVAL {
                info_checked = Instant::now();
                if let Some(kind) = api.kind() {
                    let new_info = api.info(kind);
                    if info.as_ref() != Some(&new_info) {
                        info = Some(new_info.clone());
                        let _ =
                            events.send(VmEvent::Status(RemoteStatus::Connected, Some(new_info)));
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::*;

    pub fn run(rx: std_mpsc::Receiver<VmRequest>, events: UnboundedSender<VmEvent>) {
        let _ = events.send(VmEvent::Status(RemoteStatus::Unsupported, None));
        while let Ok(request) = rx.recv() {
            if let VmRequest::Shutdown = request {
                return;
            }
        }
    }
}
