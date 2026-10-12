//! Uses existing WASAPI session metadata only. Never starts an audio client.
use super::{application, meet_code, Application, Observation};
use std::{
    collections::BTreeSet,
    hash::{Hash, Hasher},
};
use windows::{
    core::{Interface, BOOL, PWSTR},
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM},
        Media::Audio::*,
        System::{Com::*, Threading::*},
        UI::WindowsAndMessaging::*,
    },
};
struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
fn executable(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut length = buffer.len() as u32;
        let outcome = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(handle);
        outcome.ok()?;
        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        Some(path.rsplit(['\\', '/']).next()?.to_ascii_lowercase())
    }
}
fn active_microphones() -> Result<BTreeSet<String>, String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|_| "Microphone activity metadata is unavailable.")?;
        let endpoints = enumerator
            .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
            .map_err(|_| "Microphone activity metadata is unavailable.")?;
        let mut active = BTreeSet::new();
        for index in 0..endpoints
            .GetCount()
            .map_err(|_| "Microphone activity metadata is unavailable.")?
        {
            let endpoint = endpoints
                .Item(index)
                .map_err(|_| "Microphone activity metadata is unavailable.")?;
            let manager: IAudioSessionManager2 = endpoint
                .Activate(CLSCTX_ALL, None)
                .map_err(|_| "Microphone activity metadata is unavailable.")?;
            let sessions = manager
                .GetSessionEnumerator()
                .map_err(|_| "Microphone activity metadata is unavailable.")?;
            for session_index in 0..sessions
                .GetCount()
                .map_err(|_| "Microphone activity metadata is unavailable.")?
            {
                let session = sessions
                    .GetSession(session_index)
                    .map_err(|_| "Microphone activity metadata is unavailable.")?;
                if session.GetState().ok() != Some(AudioSessionStateActive) {
                    continue;
                }
                let Ok(control) = session.cast::<IAudioSessionControl2>() else {
                    continue;
                };
                let Ok(pid) = control.GetProcessId() else {
                    continue;
                };
                if let Some(exe) = executable(pid) {
                    if application(&exe).is_some() {
                        active.insert(exe);
                    }
                }
            }
        }
        Ok(active)
    }
}
struct Scan {
    active: BTreeSet<String>,
    windows: Vec<Observation>,
}
unsafe extern "system" fn visit(hwnd: HWND, parameter: LPARAM) -> BOOL {
    let scan = &mut *(parameter.0 as *mut Scan);
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    let Some(executable) = executable(pid) else {
        return BOOL(1);
    };
    let Some(application) = application(&executable) else {
        return BOOL(1);
    };
    // Read only supported apps' top-level titles, transiently. No UI Automation,
    // URLs, tab contents, meeting titles, PID lists, or paths leave this adapter.
    let mut buffer = [0u16; 512];
    let length = GetWindowTextW(hwnd, &mut buffer);
    if length <= 0 {
        return BOOL(1);
    }
    let title = String::from_utf16_lossy(&buffer[..length as usize]);
    let code = if application == Application::GoogleMeet {
        meet_code(&title)
    } else {
        None
    };
    if application == Application::GoogleMeet && code.is_none() {
        return BOOL(1);
    }
    let meeting_window = match application {
        Application::Zoom => title == "Zoom Meeting" || title == "Zoom Webinar",
        Application::GoogleMeet => code.is_some(),
        Application::Teams => false,
    };
    let mut key = std::collections::hash_map::DefaultHasher::new();
    hwnd.0.hash(&mut key);
    pid.hash(&mut key);
    application.hash(&mut key);
    code.hash(&mut key);
    scan.windows.push(Observation {
        key: key.finish(),
        application,
        meeting_window,
        microphone_active: scan.active.contains(&executable),
    });
    BOOL(1)
}
pub fn scan() -> Result<Vec<Observation>, String> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
        .map_err(|_| "Windows meeting detection is unavailable.")?;
    let _com = Com;
    // Missing activity metadata degrades to Zoom's explicit call-window heuristic.
    let active = active_microphones().unwrap_or_default();
    let mut scan = Scan {
        active,
        windows: vec![],
    };
    unsafe { EnumWindows(Some(visit), LPARAM((&mut scan as *mut Scan) as isize)) }
        .map_err(|_| "Meeting window identification is unavailable.")?;
    Ok(scan.windows)
}
