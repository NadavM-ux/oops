//! macOS: a CGEventTap sees every key (needs the Accessibility permission),
//! CGEventPost types the fix, NSPasteboard handles selections and the Text Input
//! Sources API switches the keyboard layout.

use super::{Permission, Sound, MAGIC, SOUND_FIXED, SOUND_NOTHING};
use crate::convert::Lang;
use crate::engine::{KeyEvent, KeyKind, Sink, StartError};
use crate::hotkey::{self, Mods};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::{CFBoolean, CFBooleanRef};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::url::{CFURL, CFURLRef};
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CGEventSourceRef = *mut c_void;
type TapCallback = extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, options: u32, mask: u64, callback: TapCallback, info: *mut c_void) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventKeyboardGetUnicodeString(event: CGEventRef, max: usize, actual: *mut usize, buf: *mut u16);
    fn CGEventKeyboardSetUnicodeString(event: CGEventRef, len: usize, buf: *const u16);
    fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, keycode: u16, down: bool) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
    fn CGEventSourceSetUserData(source: CGEventSourceRef, data: i64);
    fn CGEventSourceSetLocalEventsSuppressionInterval(source: CGEventSourceRef, seconds: f64);
    fn CGEventSourceFlagsState(state: i32) -> u64;
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(alloc: *const c_void, port: CFMachPortRef, order: isize) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRelease(cf: *const c_void);
    static kCFRunLoopCommonModes: CFStringRef;
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    fn TISCreateInputSourceList(properties: CFDictionaryRef, include_all_installed: u8) -> CFArrayRef;
    fn TISGetInputSourceProperty(source: *const c_void, key: CFStringRef) -> *const c_void;
    fn TISSelectInputSource(source: *const c_void) -> i32;
    static kTISPropertyInputSourceCategory: CFStringRef;
    static kTISCategoryKeyboardInputSource: CFStringRef;
    static kTISPropertyInputSourceIsSelectCapable: CFStringRef;
    static kTISPropertyInputSourceLanguages: CFStringRef;
}

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioServicesCreateSystemSoundID(url: CFURLRef, out: *mut u32) -> i32;
    fn AudioServicesPlaySystemSound(id: u32);
}

extern "C" {
    static _dispatch_main_q: u8;
    fn dispatch_sync_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn pthread_main_np() -> i32;
}

// CGEventTapLocation / placement / options
const SESSION_TAP: u32 = 1;
const HID_TAP: u32 = 0;
const HEAD_INSERT: u32 = 0;
const TAP_DEFAULT: u32 = 0;
// CGEventType
const LEFT_MOUSE_DOWN: u32 = 1;
const RIGHT_MOUSE_DOWN: u32 = 3;
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const OTHER_MOUSE_DOWN: u32 = 25;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;
// CGEventField
const FIELD_AUTOREPEAT: u32 = 8;
const FIELD_KEYCODE: u32 = 9;
const FIELD_TARGET_PID: u32 = 40;
const FIELD_USER_DATA: u32 = 42;
// CGEventFlags
const FLAG_SHIFT: u64 = 0x0002_0000;
const FLAG_CONTROL: u64 = 0x0004_0000;
const FLAG_OPTION: u64 = 0x0008_0000;
const FLAG_COMMAND: u64 = 0x0010_0000;
const MODIFIER_FLAGS: u64 = FLAG_SHIFT | FLAG_CONTROL | FLAG_OPTION | FLAG_COMMAND;
// CGEventSourceStateID
const STATE_PRIVATE: i32 = -1;
const STATE_HID: i32 = 1;
// Virtual key codes
const VK_DELETE: u16 = 0x33;
const VK_COMMAND: u16 = 0x37;
const VK_C: u16 = 0x08;
const VK_V: u16 = 0x09;
const VK_ANSI_A: u16 = 0x00;

static SINK: OnceLock<Arc<dyn Sink>> = OnceLock::new();
static TAP: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// Runs `f` on the main thread (AppKit and Text Input Sources insist on it).
pub(crate) fn on_main<R: Send, F: FnOnce() -> R + Send>(f: F) -> R {
    if unsafe { pthread_main_np() } != 0 {
        return f();
    }
    struct Slot<F, R> {
        f: Option<F>,
        r: Option<R>,
    }
    extern "C" fn trampoline<F: FnOnce() -> R, R>(ctx: *mut c_void) {
        let slot = unsafe { &mut *(ctx as *mut Slot<F, R>) };
        if let Some(f) = slot.f.take() {
            slot.r = Some(f());
        }
    }
    let mut slot = Slot { f: Some(f), r: None };
    unsafe {
        dispatch_sync_f(
            ptr::addr_of!(_dispatch_main_q) as *const c_void,
            &mut slot as *mut Slot<F, R> as *mut c_void,
            trampoline::<F, R>,
        );
    }
    slot.r.expect("main-thread closure ran")
}

pub(crate) fn key_code(js_code: &str) -> Option<u32> {
    hotkey::key_def(js_code).map(|k| k.mac as u32)
}

// ---------------------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------------------

fn is_function_key(code: u16) -> bool {
    matches!(
        code,
        0x7A | 0x78 | 0x63 | 0x76 | 0x60 | 0x61 | 0x62 | 0x64 | 0x65 | 0x6D | 0x67 | 0x6F | 0x69 | 0x6B | 0x71
            | 0x6A | 0x40 | 0x4F | 0x50 | 0x5A
    )
}

unsafe fn typed_text(event: CGEventRef) -> String {
    let mut buf = [0u16; 8];
    let mut len = 0usize;
    CGEventKeyboardGetUnicodeString(event, buf.len(), &mut len, buf.as_mut_ptr());
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

unsafe fn classify(code: u16, mods: Mods, event: CGEventRef) -> KeyKind {
    match code {
        VK_DELETE if mods.alt || mods.meta || mods.ctrl => KeyKind::Other, // delete word / line
        VK_DELETE => KeyKind::Backspace,
        // Return, Tab, Esc, keypad Enter, forward delete, Home/End/Page Up/Page Down, arrows, Help
        0x24 | 0x30 | 0x35 | 0x4C | 0x75 | 0x73 | 0x77 | 0x74 | 0x79 | 0x7B..=0x7E | 0x72 => KeyKind::Other,
        // Command, Shift, Caps Lock, Option, Control (left/right), Fn
        0x36..=0x3F => KeyKind::Modifier,
        _ if mods.meta || mods.ctrl || is_function_key(code) => KeyKind::Other,
        _ => {
            let text = typed_text(event);
            if text.is_empty() {
                KeyKind::Modifier // dead key: the next key produces the character
            } else if text.chars().any(|c| c.is_control()) {
                KeyKind::Other
            } else {
                KeyKind::Text(text)
            }
        }
    }
}

extern "C" fn tap_callback(_proxy: *mut c_void, etype: u32, event: CGEventRef, _info: *mut c_void) -> CGEventRef {
    let result = std::panic::catch_unwind(|| unsafe { handle_event(etype, event) });
    match result {
        Ok(true) => ptr::null_mut(),
        _ => event,
    }
}

/// Returns true when the event must be swallowed.
unsafe fn handle_event(etype: u32, event: CGEventRef) -> bool {
    if etype == TAP_DISABLED_BY_TIMEOUT || etype == TAP_DISABLED_BY_USER_INPUT {
        let tap = TAP.load(Ordering::SeqCst);
        if !tap.is_null() {
            CGEventTapEnable(tap, true);
        }
        return false;
    }
    let Some(sink) = SINK.get() else { return false };
    if CGEventGetIntegerValueField(event, FIELD_USER_DATA) == MAGIC {
        return false; // our own synthetic keystrokes
    }
    match etype {
        LEFT_MOUSE_DOWN | RIGHT_MOUSE_DOWN | OTHER_MOUSE_DOWN => {
            sink.mouse_down();
            false
        }
        KEY_DOWN => {
            let code = CGEventGetIntegerValueField(event, FIELD_KEYCODE) as u16;
            let flags = CGEventGetFlags(event);
            let mods = Mods {
                ctrl: flags & FLAG_CONTROL != 0,
                alt: flags & FLAG_OPTION != 0,
                shift: flags & FLAG_SHIFT != 0,
                meta: flags & FLAG_COMMAND != 0,
            };
            let ev = KeyEvent {
                code: code as u32,
                mods,
                kind: classify(code, mods, event),
                target: CGEventGetIntegerValueField(event, FIELD_TARGET_PID) as u64,
                repeat: CGEventGetIntegerValueField(event, FIELD_AUTOREPEAT) != 0,
            };
            sink.key_down(&ev)
        }
        KEY_UP => {
            let code = CGEventGetIntegerValueField(event, FIELD_KEYCODE) as u32;
            sink.key_up(code)
        }
        _ => false,
    }
}

pub(crate) fn start_hook(sink: Arc<dyn Sink>) -> Result<(), StartError> {
    if !unsafe { AXIsProcessTrusted() } {
        return Err(StartError::NoPermission);
    }
    let _ = SINK.set(sink);
    let (tx, rx) = std::sync::mpsc::sync_channel::<bool>(1);
    std::thread::Builder::new()
        .name("oops-hook".into())
        .spawn(move || unsafe {
            let mask: u64 = (1 << KEY_DOWN) | (1 << KEY_UP) | (1 << LEFT_MOUSE_DOWN) | (1 << RIGHT_MOUSE_DOWN) | (1 << OTHER_MOUSE_DOWN);
            let tap = CGEventTapCreate(SESSION_TAP, HEAD_INSERT, TAP_DEFAULT, mask, tap_callback, ptr::null_mut());
            if tap.is_null() {
                let _ = tx.send(false);
                return;
            }
            let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
            CFRunLoopAddSource(CFRunLoopGetCurrent(), source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);
            TAP.store(tap, Ordering::SeqCst);
            let _ = tx.send(true);
            CFRunLoopRun();
            TAP.store(ptr::null_mut(), Ordering::SeqCst);
        })
        .map_err(|_| StartError::Failed)?;
    match rx.recv_timeout(Duration::from_secs(3)) {
        Ok(true) => Ok(()),
        _ => Err(StartError::Failed),
    }
}

pub(crate) fn hook_running() -> bool {
    let tap = TAP.load(Ordering::SeqCst);
    !tap.is_null() && unsafe { CGEventTapIsEnabled(tap) } && unsafe { AXIsProcessTrusted() }
}

// ---------------------------------------------------------------------------------------
// Synthetic keystrokes
// ---------------------------------------------------------------------------------------

struct Source(CGEventSourceRef);
unsafe impl Send for Source {}
unsafe impl Sync for Source {}

fn source() -> CGEventSourceRef {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    SOURCE
        .get_or_init(|| unsafe {
            let s = CGEventSourceCreate(STATE_PRIVATE);
            if !s.is_null() {
                CGEventSourceSetUserData(s, MAGIC);
                CGEventSourceSetLocalEventsSuppressionInterval(s, 0.0);
            }
            Source(s)
        })
        .0
}

fn post_key(code: u16, down: bool, flags: u64, text: Option<&[u16]>) {
    unsafe {
        let ev = CGEventCreateKeyboardEvent(source(), code, down);
        if ev.is_null() {
            return;
        }
        CGEventSetFlags(ev, flags);
        if let Some(units) = text {
            CGEventKeyboardSetUnicodeString(ev, units.len(), units.as_ptr());
        }
        CGEventSetIntegerValueField(ev, FIELD_USER_DATA, MAGIC);
        CGEventPost(HID_TAP, ev);
        CFRelease(ev as *const c_void);
    }
}

fn tap_key(code: u16, flags: u64) {
    post_key(code, true, flags, None);
    post_key(code, false, flags, None);
}

pub(crate) fn wait_modifiers_released(timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if unsafe { CGEventSourceFlagsState(STATE_HID) } & MODIFIER_FLAGS == 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}

/// Windows needs this to stop Alt from opening menus; macOS doesn't.
pub(crate) fn send_mask_key() {}

pub(crate) fn send_backspaces(n: usize) {
    for _ in 0..n {
        tap_key(VK_DELETE, 0);
        std::thread::sleep(Duration::from_millis(2));
    }
    if n > 0 {
        std::thread::sleep(Duration::from_millis(15));
    }
}

pub(crate) fn type_text(text: &str) {
    let mut buf = [0u16; 2];
    for c in text.chars() {
        let units = c.encode_utf16(&mut buf);
        post_key(VK_ANSI_A, true, 0, Some(units));
        post_key(VK_ANSI_A, false, 0, Some(units));
        std::thread::sleep(Duration::from_millis(3));
    }
}

fn command_chord(key: u16) {
    post_key(VK_COMMAND, true, FLAG_COMMAND, None);
    post_key(key, true, FLAG_COMMAND, None);
    post_key(key, false, FLAG_COMMAND, None);
    post_key(VK_COMMAND, false, 0, None);
}

pub(crate) fn send_copy() {
    command_chord(VK_C);
}

pub(crate) fn send_paste() {
    command_chord(VK_V);
}

// ---------------------------------------------------------------------------------------
// Keyboard layout (Text Input Sources)
// ---------------------------------------------------------------------------------------

/// Finds an enabled keyboard input source whose primary language matches `lang`.
unsafe fn find_input_source(lang: Lang) -> Option<CFType> {
    let category_key = CFString::wrap_under_get_rule(kTISPropertyInputSourceCategory);
    let category_val = CFString::wrap_under_get_rule(kTISCategoryKeyboardInputSource);
    let filter = CFDictionary::from_CFType_pairs(&[(category_key.as_CFType(), category_val.as_CFType())]);
    let list_ref = TISCreateInputSourceList(filter.as_concrete_TypeRef(), 0);
    if list_ref.is_null() {
        return None;
    }
    let list: CFArray<CFType> = CFArray::wrap_under_create_rule(list_ref);
    let prefix = match lang {
        Lang::Hebrew => "he",
        Lang::English => "en",
    };
    for src in list.iter() {
        let src_ref = src.as_CFTypeRef();
        let capable = TISGetInputSourceProperty(src_ref, kTISPropertyInputSourceIsSelectCapable);
        if capable.is_null() || !bool::from(CFBoolean::wrap_under_get_rule(capable as CFBooleanRef)) {
            continue;
        }
        let langs = TISGetInputSourceProperty(src_ref, kTISPropertyInputSourceLanguages);
        if langs.is_null() {
            continue;
        }
        let langs: CFArray<CFString> = CFArray::wrap_under_get_rule(langs as CFArrayRef);
        let first = langs.iter().next().map(|s| s.to_string()).unwrap_or_default();
        if first == prefix || first.starts_with(&format!("{prefix}-")) {
            return Some(src.clone());
        }
    }
    None
}

pub(crate) fn switch_layout(lang: Lang) {
    on_main(move || unsafe {
        if let Some(src) = find_input_source(lang) {
            TISSelectInputSource(src.as_CFTypeRef());
        }
    });
}

pub fn has_layout(lang: Lang) -> bool {
    on_main(move || unsafe { find_input_source(lang).is_some() })
}

// ---------------------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------------------

pub(crate) mod clipboard {
    use super::on_main;
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
    use objc2_foundation::{NSArray, NSData, NSString};

    /// Every item on the pasteboard with every representation, as raw bytes.
    pub(crate) type Snapshot = Vec<Vec<(String, Vec<u8>)>>;

    pub(crate) fn snapshot() -> Snapshot {
        on_main(|| {
            let pb = NSPasteboard::generalPasteboard();
            let mut out = Vec::new();
            if let Some(items) = pb.pasteboardItems() {
                for item in items.iter() {
                    let mut reps = Vec::new();
                    for ty in item.types().iter() {
                        if let Some(data) = item.dataForType(&ty) {
                            reps.push((ty.to_string(), data.to_vec()));
                        }
                    }
                    out.push(reps);
                }
            }
            out
        })
    }

    pub(crate) fn restore(saved: Snapshot) {
        on_main(move || {
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();
            if saved.is_empty() {
                return;
            }
            let items: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = saved
                .iter()
                .map(|reps| {
                    let item = NSPasteboardItem::new();
                    for (ty, bytes) in reps {
                        let data = NSData::with_bytes(bytes);
                        item.setData_forType(&data, &NSString::from_str(ty));
                    }
                    ProtocolObject::from_retained(item)
                })
                .collect();
            pb.writeObjects(&NSArray::from_retained_slice(&items));
        })
    }

    pub(crate) fn change_count() -> isize {
        on_main(|| NSPasteboard::generalPasteboard().changeCount())
    }

    pub(crate) fn text() -> Option<String> {
        on_main(|| {
            let pb = NSPasteboard::generalPasteboard();
            pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string())
        })
    }

    /// Writes our temporary text, marked so clipboard managers ignore it
    /// (see nspasteboard.org).
    pub(crate) fn set_text(text: &str) {
        let text = text.to_owned();
        on_main(move || {
            let pb = NSPasteboard::generalPasteboard();
            pb.clearContents();
            let item = NSPasteboardItem::new();
            item.setString_forType(&NSString::from_str(&text), unsafe { NSPasteboardTypeString });
            let empty = NSData::new();
            for marker in ["org.nspasteboard.TransientType", "org.nspasteboard.AutoGeneratedType"] {
                item.setData_forType(&empty, &NSString::from_str(marker));
            }
            let items = [ProtocolObject::from_retained(item)];
            pb.writeObjects(&NSArray::from_retained_slice(&items));
        })
    }

    /// VS Code-style editors copy the whole line when nothing is selected and say so
    /// in their private clipboard data.
    pub(crate) fn is_empty_selection_copy() -> bool {
        on_main(|| {
            let pb = NSPasteboard::generalPasteboard();
            let Some(items) = pb.pasteboardItems() else { return false };
            for item in items.iter() {
                for ty in item.types().iter() {
                    let name = ty.to_string();
                    if name.contains("chromium") || name.contains("vscode") {
                        if let Some(data) = item.dataForType(&ty) {
                            if super::super::contains_empty_selection_marker(&data.to_vec()) {
                                return true;
                            }
                        }
                    }
                }
            }
            false
        })
    }
}

// ---------------------------------------------------------------------------------------
// Permission & sounds
// ---------------------------------------------------------------------------------------

pub fn permission() -> Permission {
    if unsafe { AXIsProcessTrusted() } {
        Permission::Granted
    } else {
        Permission::Missing
    }
}

/// Shows the system "would like to control this computer" prompt (once per app identity).
pub fn request_permission() {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let opts = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
        AXIsProcessTrustedWithOptions(opts.as_concrete_TypeRef());
    }
}

pub fn open_permission_settings() {
    let _ = std::process::Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// After an update the app's signature changes and macOS keeps a stale, non-working
/// entry; resetting it lets the user simply switch Oops on again.
pub fn reset_permission(bundle_id: &str) {
    let _ = std::process::Command::new("/usr/bin/tccutil")
        .args(["reset", "Accessibility", bundle_id])
        .status();
}

static SOUND_IDS: Mutex<Option<(u32, u32)>> = Mutex::new(None);

fn sound_ids() -> Option<(u32, u32)> {
    let mut guard = SOUND_IDS.lock().ok()?;
    if guard.is_none() {
        let dir = std::env::temp_dir().join("oops-sounds");
        std::fs::create_dir_all(&dir).ok()?;
        let make = |name: &str, bytes: &[u8]| -> Option<u32> {
            let path = dir.join(name);
            std::fs::write(&path, bytes).ok()?;
            let url = CFURL::from_path(&path, false)?;
            let mut id = 0u32;
            let status = unsafe { AudioServicesCreateSystemSoundID(url.as_concrete_TypeRef(), &mut id) };
            (status == 0).then_some(id)
        };
        *guard = Some((make("fixed.wav", SOUND_FIXED)?, make("nothing.wav", SOUND_NOTHING)?));
    }
    *guard
}

pub fn play_sound(sound: Sound) {
    if let Some((fixed, nothing)) = sound_ids() {
        unsafe {
            AudioServicesPlaySystemSound(match sound {
                Sound::Fixed => fixed,
                Sound::Nothing => nothing,
            })
        };
    }
}
