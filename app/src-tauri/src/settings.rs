//! User settings, stored as JSON next to the app's other data:
//! macOS `~/Library/Application Support/tech.nadav.oops/settings.json`,
//! Windows `%APPDATA%\tech.nadav.oops\settings.json`.

use oops_core::{EngineConfig, Hotkey};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub hotkey: Hotkey,
    /// false = paused from the tray.
    pub enabled: bool,
    pub switch_layout: bool,
    pub sounds: bool,
    pub launch_at_login: bool,
    pub check_updates: bool,
    /// "he" or "en".
    pub language: String,
    pub onboarded: bool,
    pub fix_count: u64,
    /// macOS: permission was granted at some point (detects updates that invalidate it).
    pub had_permission: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            hotkey: Hotkey::default(),
            enabled: true,
            switch_layout: true,
            sounds: true,
            launch_at_login: true,
            check_updates: true,
            language: "he".into(),
            onboarded: false,
            fix_count: 0,
            had_permission: false,
        }
    }
}

/// Partial update sent by the settings window.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub hotkey: Option<Hotkey>,
    pub enabled: Option<bool>,
    pub switch_layout: Option<bool>,
    pub sounds: Option<bool>,
    pub launch_at_login: Option<bool>,
    pub check_updates: Option<bool>,
    pub language: Option<String>,
}

impl Settings {
    pub fn path(config_dir: &Path) -> PathBuf {
        config_dir.join("settings.json")
    }

    pub fn load(config_dir: &Path) -> Settings {
        std::fs::read_to_string(Self::path(config_dir))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, config_dir: &Path) {
        if std::fs::create_dir_all(config_dir).is_ok() {
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let tmp = config_dir.join("settings.json.tmp");
                if std::fs::write(&tmp, json).is_ok() {
                    let _ = std::fs::rename(&tmp, Self::path(config_dir));
                }
            }
        }
    }

    pub fn engine_config(&self) -> EngineConfig {
        EngineConfig { hotkey: self.hotkey.clone(), enabled: self.enabled, switch_layout: self.switch_layout }
    }

    pub fn is_hebrew(&self) -> bool {
        self.language != "en"
    }
}

/// Short human label for the shortcut: `⌥O` on macOS, `Alt+O` on Windows.
pub fn hotkey_label(hk: &Hotkey) -> String {
    let key = key_label(&hk.code);
    if cfg!(target_os = "macos") {
        let mut s = String::new();
        if hk.ctrl {
            s.push('⌃');
        }
        if hk.alt {
            s.push('⌥');
        }
        if hk.shift {
            s.push('⇧');
        }
        if hk.meta {
            s.push('⌘');
        }
        s + &key
    } else {
        let mut parts: Vec<&str> = Vec::new();
        if hk.ctrl {
            parts.push("Ctrl");
        }
        if hk.alt {
            parts.push("Alt");
        }
        if hk.shift {
            parts.push("Shift");
        }
        if hk.meta {
            parts.push("Win");
        }
        let mut s = parts.join("+");
        if !s.is_empty() {
            s.push('+');
        }
        s + &key
    }
}

fn key_label(code: &str) -> String {
    if let Some(letter) = code.strip_prefix("Key") {
        return letter.to_string();
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return digit.to_string();
    }
    match code {
        "Backquote" => "`",
        "Minus" => "-",
        "Equal" => "=",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Backslash" => "\\",
        "Semicolon" => ";",
        "Quote" => "'",
        "Comma" => ",",
        "Period" => ".",
        "Slash" => "/",
        "IntlBackslash" => "§",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_label() {
        let label = hotkey_label(&Hotkey::default());
        assert!(label == "⌥O" || label == "Alt+O", "{label}");
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"sounds":false}"#).unwrap();
        assert!(!s.sounds);
        assert!(s.switch_layout);
        assert_eq!(s.hotkey, Hotkey::default());
    }
}
