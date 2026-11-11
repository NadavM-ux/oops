//! Stub for unsupported platforms (lets the pure logic compile and test anywhere).

use super::{Permission, Sound};
use crate::convert::Lang;
use crate::engine::{Sink, StartError};
use std::sync::Arc;
use std::time::Duration;

pub(crate) fn key_code(_js_code: &str) -> Option<u32> { None }
pub(crate) fn start_hook(_sink: Arc<dyn Sink>) -> Result<(), StartError> { Err(StartError::Failed) }
pub(crate) fn hook_running() -> bool { false }
pub(crate) fn wait_modifiers_released(_timeout: Duration) {}
pub(crate) fn send_mask_key() {}
pub(crate) fn send_backspaces(_n: usize) {}
pub(crate) fn type_text(_text: &str) {}
pub(crate) fn send_copy() {}
pub(crate) fn send_paste() {}
pub(crate) fn switch_layout(_lang: Lang) {}
pub fn has_layout(_lang: Lang) -> bool { false }
pub fn permission() -> Permission { Permission::Missing }
pub fn request_permission() {}
pub fn open_permission_settings() {}
pub fn reset_permission(_bundle_id: &str) {}
pub fn play_sound(_sound: Sound) {}

pub(crate) mod clipboard {
    pub(crate) type Snapshot = ();
    pub(crate) fn snapshot() -> Snapshot {}
    pub(crate) fn restore(_saved: Snapshot) {}
    pub(crate) fn change_count() -> isize { 0 }
    pub(crate) fn text() -> Option<String> { None }
    pub(crate) fn set_text(_text: &str) {}
    pub(crate) fn is_empty_selection_copy() -> bool { false }
}
