//! OS-specific pieces: keyboard/mouse hook, synthetic keystrokes, clipboard,
//! keyboard-layout switching, permissions and sounds.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    Granted,
    Missing,
    /// The OS doesn't require any permission (Windows).
    NotNeeded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Fixed,
    Nothing,
}

pub(crate) static SOUND_FIXED: &[u8] = include_bytes!("../../assets/fixed.wav");
pub(crate) static SOUND_NOTHING: &[u8] = include_bytes!("../../assets/nothing.wav");

/// Value stamped on every event we synthesize, so our own hook ignores it.
pub(crate) const MAGIC: i64 = 0x0095_0E95;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(any(target_os = "macos", windows)))]
mod other;
#[cfg(not(any(target_os = "macos", windows)))]
pub use other::*;

/// VS Code marks line copies made with an empty selection with
/// `"isFromEmptySelection":true` inside Chromium's custom clipboard data
/// (UTF-8 or UTF-16LE depending on the OS).
pub(crate) fn contains_empty_selection_marker(bytes: &[u8]) -> bool {
    const MARKER: &str = "\"isFromEmptySelection\":true";
    let utf8 = MARKER.as_bytes();
    let utf16: Vec<u8> = MARKER.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    bytes.windows(utf8.len()).any(|w| w == utf8) || bytes.windows(utf16.len()).any(|w| w == utf16.as_slice())
}

#[cfg(test)]
mod tests {
    #[test]
    fn finds_vscode_marker_in_both_encodings() {
        let json = r#"{"version":1,"isFromEmptySelection":true,"mode":"rust"}"#;
        assert!(super::contains_empty_selection_marker(json.as_bytes()));
        let utf16: Vec<u8> = json.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert!(super::contains_empty_selection_marker(&utf16));
        assert!(!super::contains_empty_selection_marker(br#"{"isFromEmptySelection":false}"#));
    }
}
