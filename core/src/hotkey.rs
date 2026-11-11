//! The global shortcut: stored with browser `KeyboardEvent.code` names (so the settings
//! UI can record it), mapped to physical key codes for each OS.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hotkey {
    /// `KeyboardEvent.code`, e.g. `"KeyO"`.
    pub code: String,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    /// Command on macOS, the Windows key on Windows.
    #[serde(default)]
    pub meta: bool,
}

impl Default for Hotkey {
    fn default() -> Self {
        Hotkey { code: "KeyO".into(), ctrl: false, alt: true, shift: false, meta: false }
    }
}

impl Hotkey {
    pub fn mods(&self) -> Mods {
        Mods { ctrl: self.ctrl, alt: self.alt, shift: self.shift, meta: self.meta }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// A physical key: `KeyboardEvent.code`, macOS virtual key code, Windows scan code.
pub struct KeyDef {
    pub code: &'static str,
    pub mac: u16,
    pub win_scan: u16,
}

pub const KEYS: &[KeyDef] = &[
    KeyDef { code: "KeyA", mac: 0x00, win_scan: 0x1E },
    KeyDef { code: "KeyB", mac: 0x0B, win_scan: 0x30 },
    KeyDef { code: "KeyC", mac: 0x08, win_scan: 0x2E },
    KeyDef { code: "KeyD", mac: 0x02, win_scan: 0x20 },
    KeyDef { code: "KeyE", mac: 0x0E, win_scan: 0x12 },
    KeyDef { code: "KeyF", mac: 0x03, win_scan: 0x21 },
    KeyDef { code: "KeyG", mac: 0x05, win_scan: 0x22 },
    KeyDef { code: "KeyH", mac: 0x04, win_scan: 0x23 },
    KeyDef { code: "KeyI", mac: 0x22, win_scan: 0x17 },
    KeyDef { code: "KeyJ", mac: 0x26, win_scan: 0x24 },
    KeyDef { code: "KeyK", mac: 0x28, win_scan: 0x25 },
    KeyDef { code: "KeyL", mac: 0x25, win_scan: 0x26 },
    KeyDef { code: "KeyM", mac: 0x2E, win_scan: 0x32 },
    KeyDef { code: "KeyN", mac: 0x2D, win_scan: 0x31 },
    KeyDef { code: "KeyO", mac: 0x1F, win_scan: 0x18 },
    KeyDef { code: "KeyP", mac: 0x23, win_scan: 0x19 },
    KeyDef { code: "KeyQ", mac: 0x0C, win_scan: 0x10 },
    KeyDef { code: "KeyR", mac: 0x0F, win_scan: 0x13 },
    KeyDef { code: "KeyS", mac: 0x01, win_scan: 0x1F },
    KeyDef { code: "KeyT", mac: 0x11, win_scan: 0x14 },
    KeyDef { code: "KeyU", mac: 0x20, win_scan: 0x16 },
    KeyDef { code: "KeyV", mac: 0x09, win_scan: 0x2F },
    KeyDef { code: "KeyW", mac: 0x0D, win_scan: 0x11 },
    KeyDef { code: "KeyX", mac: 0x07, win_scan: 0x2D },
    KeyDef { code: "KeyY", mac: 0x10, win_scan: 0x15 },
    KeyDef { code: "KeyZ", mac: 0x06, win_scan: 0x2C },
    KeyDef { code: "Digit1", mac: 0x12, win_scan: 0x02 },
    KeyDef { code: "Digit2", mac: 0x13, win_scan: 0x03 },
    KeyDef { code: "Digit3", mac: 0x14, win_scan: 0x04 },
    KeyDef { code: "Digit4", mac: 0x15, win_scan: 0x05 },
    KeyDef { code: "Digit5", mac: 0x17, win_scan: 0x06 },
    KeyDef { code: "Digit6", mac: 0x16, win_scan: 0x07 },
    KeyDef { code: "Digit7", mac: 0x1A, win_scan: 0x08 },
    KeyDef { code: "Digit8", mac: 0x1C, win_scan: 0x09 },
    KeyDef { code: "Digit9", mac: 0x19, win_scan: 0x0A },
    KeyDef { code: "Digit0", mac: 0x1D, win_scan: 0x0B },
    KeyDef { code: "Backquote", mac: 0x32, win_scan: 0x29 },
    KeyDef { code: "Minus", mac: 0x1B, win_scan: 0x0C },
    KeyDef { code: "Equal", mac: 0x18, win_scan: 0x0D },
    KeyDef { code: "BracketLeft", mac: 0x21, win_scan: 0x1A },
    KeyDef { code: "BracketRight", mac: 0x1E, win_scan: 0x1B },
    KeyDef { code: "Backslash", mac: 0x2A, win_scan: 0x2B },
    KeyDef { code: "Semicolon", mac: 0x29, win_scan: 0x27 },
    KeyDef { code: "Quote", mac: 0x27, win_scan: 0x28 },
    KeyDef { code: "Comma", mac: 0x2B, win_scan: 0x33 },
    KeyDef { code: "Period", mac: 0x2F, win_scan: 0x34 },
    KeyDef { code: "Slash", mac: 0x2C, win_scan: 0x35 },
    KeyDef { code: "IntlBackslash", mac: 0x0A, win_scan: 0x56 },
    KeyDef { code: "F1", mac: 0x7A, win_scan: 0x3B },
    KeyDef { code: "F2", mac: 0x78, win_scan: 0x3C },
    KeyDef { code: "F3", mac: 0x63, win_scan: 0x3D },
    KeyDef { code: "F4", mac: 0x76, win_scan: 0x3E },
    KeyDef { code: "F5", mac: 0x60, win_scan: 0x3F },
    KeyDef { code: "F6", mac: 0x61, win_scan: 0x40 },
    KeyDef { code: "F7", mac: 0x62, win_scan: 0x41 },
    KeyDef { code: "F8", mac: 0x64, win_scan: 0x42 },
    KeyDef { code: "F9", mac: 0x65, win_scan: 0x43 },
    KeyDef { code: "F10", mac: 0x6D, win_scan: 0x44 },
    KeyDef { code: "F11", mac: 0x67, win_scan: 0x57 },
    KeyDef { code: "F12", mac: 0x6F, win_scan: 0x58 },
    KeyDef { code: "F13", mac: 0x69, win_scan: 0x64 },
    KeyDef { code: "F14", mac: 0x6B, win_scan: 0x65 },
    KeyDef { code: "F15", mac: 0x71, win_scan: 0x66 },
    KeyDef { code: "F16", mac: 0x6A, win_scan: 0x67 },
    KeyDef { code: "F17", mac: 0x40, win_scan: 0x68 },
    KeyDef { code: "F18", mac: 0x4F, win_scan: 0x69 },
    KeyDef { code: "F19", mac: 0x50, win_scan: 0x6A },
];

pub fn key_def(code: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|k| k.code == code)
}

fn is_function_key(code: &str) -> bool {
    code.len() >= 2 && code.starts_with('F') && code[1..].chars().all(|c| c.is_ascii_digit())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyError {
    /// The key itself can't be used (arrows, Enter, Space, ...).
    UnsupportedKey,
    /// Letters/digits need Alt, Ctrl or Command, otherwise they'd block normal typing.
    NeedsModifier,
    /// Collides with something the OS or nearly every app already uses.
    Reserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
}

pub const CURRENT_OS: Os = if cfg!(target_os = "macos") { Os::Mac } else { Os::Windows };

/// Checks that a shortcut is usable as a global hotkey on `os`.
pub fn validate(hk: &Hotkey, os: Os) -> Result<(), HotkeyError> {
    if key_def(&hk.code).is_none() {
        return Err(HotkeyError::UnsupportedKey);
    }
    let fkey = is_function_key(&hk.code);
    let strong = match os {
        Os::Mac => hk.alt || hk.ctrl || hk.meta,
        Os::Windows => hk.alt || hk.ctrl,
    };
    if !fkey && !strong {
        return Err(HotkeyError::NeedsModifier);
    }
    // The Windows key is owned by the OS; plain Command/Ctrl+letter is owned by every app.
    if os == Os::Windows && hk.meta {
        return Err(HotkeyError::Reserved);
    }
    let only_main = |main: bool, others: [bool; 3]| main && others.iter().all(|b| !b);
    if !fkey && os == Os::Mac && only_main(hk.meta, [hk.alt, hk.ctrl, hk.shift]) {
        return Err(HotkeyError::Reserved);
    }
    if !fkey && os == Os::Windows && only_main(hk.ctrl, [hk.alt, hk.shift, hk.meta]) {
        return Err(HotkeyError::Reserved);
    }
    // Alt+F4 closes windows on Windows.
    if os == Os::Windows && hk.code == "F4" && hk.alt {
        return Err(HotkeyError::Reserved);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(code: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Hotkey {
        Hotkey { code: code.into(), ctrl, alt, shift, meta }
    }

    #[test]
    fn default_is_valid_everywhere() {
        assert_eq!(validate(&Hotkey::default(), Os::Mac), Ok(()));
        assert_eq!(validate(&Hotkey::default(), Os::Windows), Ok(()));
    }

    #[test]
    fn rejects_bad_combos() {
        assert_eq!(validate(&hk("KeyO", false, false, false, false), Os::Mac), Err(HotkeyError::NeedsModifier));
        assert_eq!(validate(&hk("KeyO", false, false, true, false), Os::Windows), Err(HotkeyError::NeedsModifier));
        assert_eq!(validate(&hk("KeyC", false, false, false, true), Os::Mac), Err(HotkeyError::Reserved));
        assert_eq!(validate(&hk("KeyC", true, false, false, false), Os::Windows), Err(HotkeyError::Reserved));
        assert_eq!(validate(&hk("KeyO", false, true, false, true), Os::Windows), Err(HotkeyError::Reserved));
        assert_eq!(validate(&hk("F4", false, true, false, false), Os::Windows), Err(HotkeyError::Reserved));
        assert_eq!(validate(&hk("Space", false, true, false, false), Os::Mac), Err(HotkeyError::UnsupportedKey));
        assert_eq!(validate(&hk("ArrowLeft", false, true, false, false), Os::Mac), Err(HotkeyError::UnsupportedKey));
    }

    #[test]
    fn accepts_good_combos() {
        assert_eq!(validate(&hk("F9", false, false, false, false), Os::Windows), Ok(()));
        assert_eq!(validate(&hk("KeyH", true, false, true, false), Os::Windows), Ok(()));
        assert_eq!(validate(&hk("KeyK", false, false, true, true), Os::Mac), Ok(()));
        assert_eq!(validate(&hk("Backquote", false, true, false, false), Os::Mac), Ok(()));
    }

    #[test]
    fn key_table_has_no_duplicates() {
        for (i, a) in KEYS.iter().enumerate() {
            for b in &KEYS[i + 1..] {
                assert_ne!(a.code, b.code);
                assert_ne!(a.mac, b.mac, "{} / {}", a.code, b.code);
                assert_ne!(a.win_scan, b.win_scan, "{} / {}", a.code, b.code);
            }
        }
    }
}
