//! Conservative, local-only classification. A signal is never proof of a meeting.
#[cfg(windows)]
mod windows;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Application {
    Teams,
    Zoom,
    GoogleMeet,
}
impl Application {
    pub fn label(self) -> &'static str {
        match self {
            Self::Teams => "Microsoft Teams",
            Self::Zoom => "Zoom",
            Self::GoogleMeet => "Google Meet",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Observation {
    pub key: u64,
    pub application: Application,
    pub meeting_window: bool,
    pub microphone_active: bool,
}
impl Observation {
    pub fn likely(&self) -> bool {
        match self.application {
            Application::Teams => self.microphone_active,
            Application::Zoom => self.meeting_window || self.microphone_active,
            Application::GoogleMeet => self.meeting_window && self.microphone_active,
        }
    }
    pub fn explanation(&self) -> &'static str {
        if self.microphone_active {
            "An identified app has an active microphone session. This is a likely meeting, not confirmation."
        } else {
            "A Zoom meeting window is visible. This is a window-based suggestion, not confirmation."
        }
    }
}
#[derive(Default)]
pub struct Policy {
    consecutive: BTreeMap<u64, u8>,
    suppressed: BTreeSet<u64>,
}
impl Policy {
    pub fn observe(&mut self, windows: &[Observation]) -> Option<Observation> {
        let present: BTreeSet<_> = windows.iter().map(|window| window.key).collect();
        self.suppressed.retain(|key| present.contains(key));
        self.consecutive.retain(|key, _| present.contains(key));
        for window in windows {
            let count = self.consecutive.entry(window.key).or_default();
            *count = if window.likely() {
                count.saturating_add(1)
            } else {
                0
            };
        }
        windows
            .iter()
            .find(|window| {
                window.likely()
                    && self.consecutive.get(&window.key).copied().unwrap_or(0) >= 2
                    && !self.suppressed.contains(&window.key)
            })
            .cloned()
    }
    pub fn dismiss(&mut self, key: u64) {
        self.suppressed.insert(key);
    }
    pub fn suppress_present(&mut self) {
        self.suppressed.extend(
            self.consecutive
                .iter()
                .filter(|(_, count)| **count > 0)
                .map(|(key, _)| *key),
        );
    }
    pub fn clear(&mut self) {
        self.consecutive.clear();
        self.suppressed.clear();
    }
}
/// Exact product identities, never a generic browser/process-presence match.
pub fn application(executable: &str) -> Option<Application> {
    match executable.to_ascii_lowercase().as_str() {
        "ms-teams.exe" | "teams.exe" => Some(Application::Teams),
        "zoom.exe" => Some(Application::Zoom),
        "chrome.exe" | "msedge.exe" => Some(Application::GoogleMeet),
        _ => None,
    }
}
/// Only a standard Meet call code in a Meet-prefixed active-tab title qualifies.
/// This code is used transiently to derive an in-memory key; never persisted.
pub fn meet_code(title: &str) -> Option<&str> {
    let title = title.trim();
    let rest = title
        .strip_prefix("Meet - ")
        .or_else(|| title.strip_prefix("Meet – "))
        .or_else(|| title.strip_prefix("Meet — "))?;
    let code = rest.get(..12)?;
    let suffix = rest.get(12..)?;
    if !suffix.is_empty() && !suffix.starts_with(' ') {
        return None;
    }
    let bytes = code.as_bytes();
    if bytes.iter().enumerate().all(|(index, byte)| {
        if index == 3 || index == 8 {
            *byte == b'-'
        } else {
            byte.is_ascii_lowercase()
        }
    }) {
        Some(code)
    } else {
        None
    }
}
pub fn scan() -> Result<Vec<Observation>, String> {
    #[cfg(windows)]
    {
        windows::scan()
    }
    #[cfg(not(windows))]
    {
        Err("Meeting detection requires Windows.".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn observation(
        application: Application,
        meeting_window: bool,
        microphone_active: bool,
    ) -> Observation {
        Observation {
            key: 1,
            application,
            meeting_window,
            microphone_active,
        }
    }
    #[test]
    fn idle_apps_and_browsers_are_not_meetings() {
        for app in [
            Application::Teams,
            Application::Zoom,
            Application::GoogleMeet,
        ] {
            assert!(!observation(app, false, false).likely());
        }
        assert!(!observation(Application::GoogleMeet, false, true).likely());
        assert!(!observation(Application::GoogleMeet, true, false).likely());
        assert_eq!(application("notepad.exe"), None);
    }
    #[test]
    fn microphone_and_window_evidence_remain_heuristics() {
        assert!(observation(Application::Teams, false, true).likely());
        assert!(observation(Application::Zoom, true, false).likely());
        assert!(observation(Application::GoogleMeet, true, true).likely());
    }
    #[test]
    fn debounce_dismiss_mute_and_reappearance() {
        let mut policy = Policy::default();
        let meeting = observation(Application::Teams, false, true);
        assert!(policy.observe(&[meeting.clone()]).is_none());
        assert!(policy.observe(&[meeting.clone()]).is_some());
        policy.dismiss(1);
        assert!(policy.observe(&[meeting.clone()]).is_none());
        assert!(policy
            .observe(&[observation(Application::Teams, false, false)])
            .is_none());
        assert!(policy.observe(&[meeting.clone()]).is_none());
        policy.observe(&[]);
        assert!(policy.observe(&[meeting.clone()]).is_none());
        assert!(policy.observe(&[meeting]).is_some());
    }
    #[test]
    fn disabling_clears_old_detection_state() {
        let mut policy = Policy::default();
        let meeting = observation(Application::Zoom, true, false);
        policy.observe(&[meeting.clone()]);
        policy.dismiss(1);
        policy.clear();
        assert!(policy.observe(&[meeting.clone()]).is_none());
        assert!(policy.observe(&[meeting]).is_some());
    }
    #[test]
    fn browser_home_and_unrelated_titles_do_not_qualify() {
        for title in [
            "Google Chrome",
            "Google Meet - Google Chrome",
            "Meet - Home - Microsoft Edge",
            "Notes abc-defg-hij - Google Chrome",
            "Meet - abc-defg-hi - Google Chrome",
            "Meet - abc-defg-hijkl - Google Chrome",
        ] {
            assert!(meet_code(title).is_none());
        }
        assert_eq!(
            meet_code("Meet – abc-defg-hij - Google Chrome"),
            Some("abc-defg-hij")
        );
        assert_eq!(
            meet_code("Meet - abc-defg-hij - Microsoft Edge"),
            Some("abc-defg-hij")
        );
    }
    #[test]
    fn manual_capture_suppresses_a_pre_prompt_observation() {
        let mut policy = Policy::default();
        let meeting = observation(Application::Zoom, true, false);
        assert!(policy.observe(&[meeting.clone()]).is_none());
        policy.suppress_present();
        assert!(policy.observe(&[meeting]).is_none());
    }
}
