//! Oops core: converts text typed in the wrong keyboard layout (English <-> Hebrew).
//!
//! * [`convert`] — the pure character mapping (unit tested, no OS calls).
//! * [`Engine`] — listens to the keyboard, catches the shortcut and replaces the text.
//! * [`system`] — permissions, layout detection and feedback sounds.

pub mod convert;
mod engine;
pub mod hotkey;
mod platform;

pub use engine::{Engine, EngineConfig, EngineEvent, StartError};
pub use hotkey::{Hotkey, HotkeyError};

/// OS integration the app layer needs directly.
pub mod system {
    pub use crate::platform::{has_layout, open_permission_settings, permission, play_sound, request_permission, reset_permission, Permission, Sound};
}
