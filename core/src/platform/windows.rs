//! Windows: low-level keyboard + mouse hooks (no special permission needed),
//! SendInput for typing, the Win32 clipboard and WM_INPUTLANGCHANGEREQUEST for layouts.

use super::{Permission, Sound, MAGIC, SOUND_FIXED, SOUND_NOTHING};
use crate::convert::Lang;
use crate::engine::{KeyEvent, KeyKind, Sink, StartError};
use crate::hotkey::{self, Mods};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const EXTRA_INFO: usize = MAGIC as usize;
const WM_INPUTLANGCHANGEREQUEST: u32 = 0x0050;
/// Unassigned virtual key; sent while Alt is held so releasing Alt doesn't open menus.
const VK_MASK: u16 = 0xE8;

static SINK: OnceLock<Arc<dyn Sink>> = OnceLock::new();
static RUNNING: AtomicBool = AtomicBool::new(false);

pub(crate) fn key_code(js_code: &str) -> Option<u32> {
    hotkey::key_def(js_code).map(|k| k.win_scan as u32)
}

fn is_down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 }
}

fn current_mods() -> Mods {
    Mods {
        ctrl: is_down(VK_CONTROL),
        alt: is_down(VK_MENU),
        shift: is_down(VK_SHIFT),
        meta: is_down(VK_LWIN) || is_down(VK_RWIN),
    }
}

// ---------------------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------------------

unsafe fn translate(vk: u32, scan: u32, mods: Mods, hkl: HKL) -> (i32, String) {
    let mut state = [0u8; 256];
    if mods.shift {
        state[VK_SHIFT.0 as usize] = 0x80;
    }
    if GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0 {
        state[VK_CAPITAL.0 as usize] = 0x01;
    }
    if mods.ctrl && mods.alt {
        // AltGr
        state[VK_CONTROL.0 as usize] = 0x80;
        state[VK_LCONTROL.0 as usize] = 0x80;
        state[VK_MENU.0 as usize] = 0x80;
        state[VK_RMENU.0 as usize] = 0x80;
    }
    let mut buf = [0u16; 8];
    // Flag 0x4: don't touch the keyboard's dead-key state (Windows 10 1607+).
    let n = ToUnicodeEx(vk, scan, &state, &mut buf, 0x4, Some(hkl));
    let text = if n > 0 { String::from_utf16_lossy(&buf[..(n as usize).min(buf.len())]) } else { String::new() };
    (n, text)
}

unsafe fn classify(vk: u16, scan: u32, mods: Mods, fg: HWND) -> KeyKind {
    let v = VIRTUAL_KEY(vk);
    match v {
        VK_BACK if mods.ctrl || mods.alt => KeyKind::Other,
        VK_BACK => KeyKind::Backspace,
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU
        | VK_CAPITAL | VK_NUMLOCK | VK_SCROLL => KeyKind::Modifier,
        _ if (VK_F1.0..=VK_F24.0).contains(&vk) || (VK_PRIOR.0..=VK_HELP.0).contains(&vk) => KeyKind::Other,
        VK_RETURN | VK_TAB | VK_ESCAPE | VK_LWIN | VK_RWIN | VK_APPS => KeyKind::Other,
        _ if mods.meta => KeyKind::Other,
        _ if mods.ctrl != mods.alt => KeyKind::Other, // Ctrl+key shortcut or Alt+key menu accelerator
        _ => {
            let thread = GetWindowThreadProcessId(fg, None);
            let hkl = GetKeyboardLayout(thread);
            let (n, text) = translate(vk as u32, scan, mods, hkl);
            if n < 0 {
                KeyKind::Modifier // dead key
            } else if text.is_empty() || text.chars().any(|c| c.is_control()) {
                KeyKind::Other
            } else {
                KeyKind::Text(text)
            }
        }
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if info.dwExtraInfo != EXTRA_INFO {
            if let Some(sink) = SINK.get() {
                let msg = wparam.0 as u32;
                let scan = info.scanCode | if info.flags.0 & LLKHF_EXTENDED.0 != 0 { 0xE000 } else { 0 };
                let swallow = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                        let mods = current_mods();
                        let fg = GetForegroundWindow();
                        let ev = KeyEvent {
                            code: scan,
                            mods,
                            kind: classify(info.vkCode as u16, info.scanCode, mods, fg),
                            target: fg.0 as u64,
                            repeat: false,
                        };
                        sink.key_down(&ev)
                    } else {
                        sink.key_up(scan)
                    }
                }))
                .unwrap_or(false);
                if swallow {
                    return LRESULT(1);
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let msg = wparam.0 as u32;
        if matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) {
            if let Some(sink) = SINK.get() {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sink.mouse_down()));
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

pub(crate) fn start_hook(sink: Arc<dyn Sink>) -> Result<(), StartError> {
    let _ = SINK.set(sink);
    let (tx, rx) = std::sync::mpsc::sync_channel::<bool>(1);
    std::thread::Builder::new()
        .name("oops-hook".into())
        .spawn(move || unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL);
            let module = GetModuleHandleW(None).ok().map(|m| HINSTANCE(m.0));
            let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), module, 0);
            let ms = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), module, 0);
            if kb.is_err() || ms.is_err() {
                let _ = tx.send(false);
                return;
            }
            RUNNING.store(true, Ordering::SeqCst);
            let _ = tx.send(true);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            RUNNING.store(false, Ordering::SeqCst);
        })
        .map_err(|_| StartError::Failed)?;
    match rx.recv_timeout(Duration::from_secs(3)) {
        Ok(true) => Ok(()),
        _ => Err(StartError::Failed),
    }
}

pub(crate) fn hook_running() -> bool {
    RUNNING.load(Ordering::SeqCst)
}

// ---------------------------------------------------------------------------------------
// Synthetic keystrokes
// ---------------------------------------------------------------------------------------

fn key_input(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: VIRTUAL_KEY(vk), wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: EXTRA_INFO },
        },
    }
}

fn vk_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16;
    key_input(vk.0, scan, if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) })
}

fn send(inputs: &[INPUT]) {
    unsafe {
        SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

pub(crate) fn wait_modifiers_released(timeout: Duration) {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let m = current_mods();
        if !(m.ctrl || m.alt || m.shift || m.meta) {
            return;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
}

pub(crate) fn send_mask_key() {
    send(&[key_input(VK_MASK, 0, KEYBD_EVENT_FLAGS(0)), key_input(VK_MASK, 0, KEYEVENTF_KEYUP)]);
}

pub(crate) fn send_backspaces(n: usize) {
    for _ in 0..n {
        send(&[vk_input(VK_BACK, false), vk_input(VK_BACK, true)]);
        std::thread::sleep(Duration::from_millis(2));
    }
    if n > 0 {
        std::thread::sleep(Duration::from_millis(15));
    }
}

pub(crate) fn type_text(text: &str) {
    for unit in text.encode_utf16() {
        send(&[key_input(0, unit, KEYEVENTF_UNICODE), key_input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn ctrl_chord(key: VIRTUAL_KEY) {
    send(&[vk_input(VK_CONTROL, false), vk_input(key, false), vk_input(key, true), vk_input(VK_CONTROL, true)]);
}

pub(crate) fn send_copy() {
    ctrl_chord(VK_C);
}

pub(crate) fn send_paste() {
    ctrl_chord(VK_V);
}

// ---------------------------------------------------------------------------------------
// Keyboard layout
// ---------------------------------------------------------------------------------------

fn installed_layout(lang: Lang) -> Option<HKL> {
    unsafe {
        let count = GetKeyboardLayoutList(None);
        if count <= 0 {
            return None;
        }
        let mut list = vec![HKL::default(); count as usize];
        let n = GetKeyboardLayoutList(Some(&mut list));
        let primary = match lang {
            Lang::Hebrew => 0x0D,
            Lang::English => 0x09,
        };
        list.into_iter().take(n.max(0) as usize).find(|h| ((h.0 as usize) & 0x3FF) == primary)
    }
}

pub(crate) fn switch_layout(lang: Lang) {
    let Some(hkl) = installed_layout(lang) else { return };
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return;
        }
        let _ = PostMessageW(Some(fg), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize));
        // Also ask the focused control (some frameworks only listen there).
        let thread = GetWindowThreadProcessId(fg, None);
        let mut gui = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        if GetGUIThreadInfo(thread, &mut gui).is_ok() && !gui.hwndFocus.0.is_null() && gui.hwndFocus != fg {
            let _ = PostMessageW(Some(gui.hwndFocus), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize));
        }
    }
}

pub fn has_layout(lang: Lang) -> bool {
    installed_layout(lang).is_some()
}

// ---------------------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------------------

pub(crate) mod clipboard {
    use std::time::Duration;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    pub(crate) type Snapshot = Vec<(u32, Vec<u8>)>;

    struct Open;
    impl Drop for Open {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    /// The clipboard is shared: another app may hold it for a moment.
    fn open() -> Option<Open> {
        for _ in 0..20 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Some(Open);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        None
    }

    /// Formats whose handle is a GDI object, not memory: they can't be copied as bytes
    /// (Windows re-synthesizes them from the DIB / text formats we do keep).
    fn is_gdi_format(f: u32) -> bool {
        matches!(f, 2 | 3 | 9 | 14 | 0x80 | 0x82 | 0x83 | 0x8E)
    }

    unsafe fn read_global(h: HANDLE) -> Option<Vec<u8>> {
        let g = HGLOBAL(h.0);
        let size = GlobalSize(g);
        if size == 0 {
            return None;
        }
        let ptr = GlobalLock(g) as *const u8;
        if ptr.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
        let _ = GlobalUnlock(g);
        Some(bytes)
    }

    unsafe fn put(format: u32, bytes: &[u8]) {
        let Ok(g) = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)) else { return };
        let ptr = GlobalLock(g) as *mut u8;
        if ptr.is_null() {
            let _ = GlobalFree(Some(g));
            return;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(g);
        if SetClipboardData(format, Some(HANDLE(g.0))).is_err() {
            let _ = GlobalFree(Some(g));
        }
    }

    pub(crate) fn snapshot() -> Snapshot {
        let mut out = Vec::new();
        let Some(_guard) = open() else { return out };
        unsafe {
            let mut format = 0u32;
            loop {
                format = EnumClipboardFormats(format);
                if format == 0 {
                    break;
                }
                if is_gdi_format(format) {
                    continue;
                }
                if let Ok(h) = GetClipboardData(format) {
                    if let Some(bytes) = read_global(h) {
                        out.push((format, bytes));
                    }
                }
            }
        }
        out
    }

    pub(crate) fn restore(saved: Snapshot) {
        let Some(_guard) = open() else { return };
        unsafe {
            let _ = EmptyClipboard();
            for (format, bytes) in &saved {
                put(*format, bytes);
            }
        }
    }

    pub(crate) fn change_count() -> isize {
        unsafe { GetClipboardSequenceNumber() as isize }
    }

    pub(crate) fn text() -> Option<String> {
        let _guard = open()?;
        unsafe {
            let h = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
            let bytes = read_global(h)?;
            let (pairs, _) = bytes.as_chunks::<2>();
            let units: Vec<u16> = pairs.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).collect();
            Some(String::from_utf16_lossy(&units))
        }
    }

    fn register(name: PCWSTR) -> u32 {
        unsafe { RegisterClipboardFormatW(name) }
    }

    /// Writes our temporary text and keeps it out of clipboard history / cloud sync.
    pub(crate) fn set_text(text: &str) {
        let Some(_guard) = open() else { return };
        unsafe {
            let _ = EmptyClipboard();
            let mut units: Vec<u16> = text.encode_utf16().collect();
            units.push(0);
            let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();
            put(CF_UNICODETEXT.0 as u32, &bytes);
            let zero = 0u32.to_le_bytes();
            put(register(w!("ExcludeClipboardContentFromMonitorProcessing")), &zero);
            put(register(w!("CanIncludeInClipboardHistory")), &zero);
            put(register(w!("CanUploadToCloudClipboard")), &zero);
        }
    }

    pub(crate) fn is_empty_selection_copy() -> bool {
        let Some(_guard) = open() else { return false };
        unsafe {
            let format = register(w!("Chromium Web Custom MIME Data Format"));
            if let Ok(h) = GetClipboardData(format) {
                if let Some(bytes) = read_global(h) {
                    return super::super::contains_empty_selection_marker(&bytes);
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------------------
// Permission & sounds
// ---------------------------------------------------------------------------------------

pub fn permission() -> Permission {
    Permission::NotNeeded
}

pub fn request_permission() {}

pub fn open_permission_settings() {}

pub fn reset_permission(_bundle_id: &str) {}

pub fn play_sound(sound: Sound) {
    let bytes: &'static [u8] = match sound {
        Sound::Fixed => SOUND_FIXED,
        Sound::Nothing => SOUND_NOTHING,
    };
    unsafe {
        let _ = PlaySoundW(PCWSTR(bytes.as_ptr() as *const u16), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}
