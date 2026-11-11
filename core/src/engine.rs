//! The engine: remembers what was just typed, catches the shortcut and performs the fix.
//!
//! Threads:
//! * the platform hook thread calls [`Sink`] for every key/mouse event (must stay fast);
//! * one worker thread performs fixes (waits for modifiers, sends keystrokes, clipboard).

use crate::convert::{self, Direction};
use crate::hotkey::{Hotkey, Mods};
use crate::platform;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Only the most recent keystrokes are kept, in memory only.
const MAX_BUFFER: usize = 300;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyKind {
    /// The key produced printable text.
    Text(String),
    /// Plain backspace: deletes one character.
    Backspace,
    /// Shift, Caps Lock, dead keys...: doesn't change the text.
    Modifier,
    /// Anything that moves the caret or leaves the field: Enter, Tab, arrows, shortcuts.
    Other,
}

#[derive(Clone, Debug)]
pub struct KeyEvent {
    /// Physical key: macOS virtual key code / Windows scan code.
    pub code: u32,
    pub mods: Mods,
    pub kind: KeyKind,
    /// The app/window receiving the key (pid on macOS, top-level window on Windows).
    pub target: u64,
    pub repeat: bool,
}

/// What the platform hook reports to the engine. Returning `true` swallows the event.
pub(crate) trait Sink: Send + Sync {
    fn key_down(&self, ev: &KeyEvent) -> bool;
    fn key_up(&self, code: u32) -> bool;
    fn mouse_down(&self);
}

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub hotkey: Hotkey,
    pub enabled: bool,
    pub switch_layout: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EngineEvent {
    #[serde(rename_all = "camelCase")]
    Fixed { before: String, after: String, dir: Direction, undo: bool },
    NothingToFix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StartError {
    NoPermission,
    Failed,
}

struct LastFix {
    inserted: String,
    removed: String,
    dir: Direction,
}

struct State {
    buffer: Vec<char>,
    target: u64,
    last_fix: Option<LastFix>,
    hotkey_code: Option<u32>,
    hotkey_mods: Mods,
    hotkey_alt_or_meta: bool,
    enabled: bool,
    switch_layout: bool,
    recording: bool,
    /// Physical code of the shortcut key while it is held (its key-up is swallowed too).
    held: Option<u32>,
}

impl State {
    fn reset(&mut self) {
        self.buffer.clear();
        self.last_fix = None;
    }

    fn apply_config(&mut self, cfg: &EngineConfig) {
        self.hotkey_code = platform::key_code(&cfg.hotkey.code);
        self.hotkey_mods = cfg.hotkey.mods();
        self.hotkey_alt_or_meta = cfg.hotkey.alt || cfg.hotkey.meta;
        self.enabled = cfg.enabled;
        self.switch_layout = cfg.switch_layout;
        self.reset();
    }

    /// Replaces the last `delete` buffered characters with `insert`.
    fn splice_tail(&mut self, delete: usize, insert: &str) {
        let keep = self.buffer.len().saturating_sub(delete);
        self.buffer.truncate(keep);
        self.buffer.extend(insert.chars());
        self.trim();
    }

    fn trim(&mut self) {
        if self.buffer.len() > MAX_BUFFER {
            let extra = self.buffer.len() - MAX_BUFFER;
            self.buffer.drain(..extra);
        }
    }
}

enum Job {
    Fix { target: u64, mask: bool },
}

struct Inner {
    state: Mutex<State>,
    busy: AtomicBool,
    jobs: Mutex<mpsc::Sender<Job>>,
    on_event: Box<dyn Fn(EngineEvent) + Send + Sync>,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Handle to the running engine. Cheap to clone.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

impl Engine {
    pub fn new(cfg: EngineConfig, on_event: impl Fn(EngineEvent) + Send + Sync + 'static) -> Engine {
        let mut state = State {
            buffer: Vec::with_capacity(MAX_BUFFER),
            target: 0,
            last_fix: None,
            hotkey_code: None,
            hotkey_mods: Mods::default(),
            hotkey_alt_or_meta: false,
            enabled: true,
            switch_layout: true,
            recording: false,
            held: None,
        };
        state.apply_config(&cfg);
        let (tx, rx) = mpsc::channel::<Job>();
        let inner = Arc::new(Inner {
            state: Mutex::new(state),
            busy: AtomicBool::new(false),
            jobs: Mutex::new(tx),
            on_event: Box::new(on_event),
        });
        let worker = inner.clone();
        std::thread::Builder::new()
            .name("oops-worker".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    match job {
                        Job::Fix { target, mask } => worker.run_fix(target, mask),
                    }
                }
            })
            .expect("spawn worker");
        Engine { inner }
    }

    /// Installs the keyboard/mouse hook. Safe to call again (e.g. after permission is granted).
    pub fn start(&self) -> Result<(), StartError> {
        if platform::hook_running() {
            return Ok(());
        }
        platform::start_hook(self.inner.clone())
    }

    pub fn is_running(&self) -> bool {
        platform::hook_running()
    }

    pub fn set_config(&self, cfg: &EngineConfig) {
        self.inner.lock().apply_config(cfg);
    }

    /// While the settings window records a new shortcut, keys must reach it untouched.
    pub fn set_recording(&self, on: bool) {
        let mut st = self.inner.lock();
        st.recording = on;
        st.reset();
    }
}

impl Sink for Inner {
    fn key_down(&self, ev: &KeyEvent) -> bool {
        let mut st = self.lock();
        if st.recording || !st.enabled {
            st.reset();
            return false;
        }
        let is_hotkey = st.hotkey_code == Some(ev.code) && st.hotkey_mods == ev.mods;
        if is_hotkey {
            if ev.repeat || st.held.is_some() || self.busy.load(Ordering::SeqCst) {
                st.held = Some(ev.code);
                return true;
            }
            st.held = Some(ev.code);
            let mask = st.hotkey_alt_or_meta;
            drop(st);
            self.busy.store(true, Ordering::SeqCst);
            let sent = self
                .jobs
                .lock()
                .map(|tx| tx.send(Job::Fix { target: ev.target, mask }).is_ok())
                .unwrap_or(false);
            if !sent {
                self.busy.store(false, Ordering::SeqCst);
            }
            return true;
        }
        if self.busy.load(Ordering::SeqCst) {
            // The user typed while we were replacing text: our picture of the field is stale.
            st.reset();
            return false;
        }
        if ev.target != st.target {
            st.reset();
            st.target = ev.target;
        }
        match &ev.kind {
            KeyKind::Text(t) => {
                st.buffer.extend(t.chars());
                st.trim();
                st.last_fix = None;
            }
            KeyKind::Backspace => {
                st.buffer.pop();
                st.last_fix = None;
            }
            KeyKind::Modifier => {}
            KeyKind::Other => st.reset(),
        }
        false
    }

    fn key_up(&self, code: u32) -> bool {
        let mut st = self.lock();
        if st.held == Some(code) {
            st.held = None;
            return true;
        }
        false
    }

    fn mouse_down(&self) {
        if !self.busy.load(Ordering::SeqCst) {
            self.lock().reset();
        }
    }
}

enum Plan {
    Undo(LastFix),
    Typed(convert::Edit),
    Selection,
}

impl Inner {
    fn run_fix(&self, target: u64, mask: bool) {
        if mask {
            platform::send_mask_key();
        }
        platform::wait_modifiers_released(Duration::from_millis(1500));
        let event = self.fix(target).unwrap_or(EngineEvent::NothingToFix);
        // Let our own synthetic events drain before we listen to the keyboard again.
        std::thread::sleep(Duration::from_millis(30));
        self.busy.store(false, Ordering::SeqCst);
        (self.on_event)(event);
    }

    fn fix(&self, target: u64) -> Option<EngineEvent> {
        let plan = {
            let mut st = self.lock();
            if st.target != target {
                st.reset();
                st.target = target;
            }
            if let Some(last) = st.last_fix.take() {
                Plan::Undo(last)
            } else {
                let typed: String = st.buffer.iter().collect();
                match convert::plan_typed(&typed) {
                    Some(edit) => Plan::Typed(edit),
                    None => Plan::Selection,
                }
            }
        };

        let (before, after, dir, undo) = match plan {
            Plan::Undo(last) => {
                let delete = last.inserted.chars().count();
                platform::send_backspaces(delete);
                platform::type_text(&last.removed);
                let dir = last.dir.reverse();
                let mut st = self.lock();
                st.splice_tail(delete, &last.removed);
                st.last_fix = Some(LastFix { inserted: last.removed.clone(), removed: last.inserted.clone(), dir });
                (last.inserted, last.removed, dir, true)
            }
            Plan::Typed(edit) => {
                platform::send_backspaces(edit.delete);
                platform::type_text(&edit.insert);
                let mut st = self.lock();
                st.splice_tail(edit.delete, &edit.insert);
                st.last_fix = Some(LastFix { inserted: edit.insert.clone(), removed: edit.removed.clone(), dir: edit.dir });
                (edit.removed, edit.insert, edit.dir, false)
            }
            Plan::Selection => {
                let (selected, replaced, dir) = self.fix_selection()?;
                let mut st = self.lock();
                st.reset();
                st.target = target;
                if !replaced.contains(['\n', '\r']) {
                    st.buffer.extend(replaced.chars());
                    st.trim();
                    st.last_fix = Some(LastFix { inserted: replaced.clone(), removed: selected.clone(), dir });
                }
                (selected, replaced, dir, false)
            }
        };

        if self.lock().switch_layout {
            platform::switch_layout(dir.target());
        }
        Some(EngineEvent::Fixed { before, after, dir, undo })
    }

    /// Copies the selection, converts it and pastes it back, then restores the clipboard.
    fn fix_selection(&self) -> Option<(String, String, Direction)> {
        let saved = platform::clipboard::snapshot();
        let before = platform::clipboard::change_count();
        platform::send_copy();
        if !wait_until(Duration::from_millis(600), || platform::clipboard::change_count() != before) {
            return None; // nothing selected: the clipboard was never touched
        }
        let result = (|| {
            if platform::clipboard::is_empty_selection_copy() {
                return None; // editors like VS Code copy the whole line when nothing is selected
            }
            let selected = platform::clipboard::text()?;
            let (replaced, dir) = convert::plan_selection(&selected)?;
            platform::clipboard::set_text(&replaced);
            platform::send_paste();
            // Give the app time to read the clipboard before we put the user's content back.
            std::thread::sleep(Duration::from_millis(350));
            Some((selected, replaced, dir))
        })();
        platform::clipboard::restore(saved);
        result
    }
}

fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if cond() {
            return true;
        }
        if start.elapsed() >= timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Arc<Inner> {
        let (tx, _rx) = mpsc::channel();
        let mut state = State {
            buffer: Vec::new(),
            target: 0,
            last_fix: None,
            hotkey_code: None,
            hotkey_mods: Mods::default(),
            hotkey_alt_or_meta: false,
            enabled: true,
            switch_layout: false,
            recording: false,
            held: None,
        };
        state.apply_config(&EngineConfig { hotkey: Hotkey::default(), enabled: true, switch_layout: false });
        Arc::new(Inner { state: Mutex::new(state), busy: AtomicBool::new(false), jobs: Mutex::new(tx), on_event: Box::new(|_| {}) })
    }

    fn text(s: &str, target: u64) -> KeyEvent {
        KeyEvent { code: 999, mods: Mods::default(), kind: KeyKind::Text(s.into()), target, repeat: false }
    }

    fn buffer(e: &Inner) -> String {
        e.lock().buffer.iter().collect()
    }

    #[test]
    fn buffer_tracks_typing_and_backspace() {
        let e = engine();
        for c in ["a", "k", "u", "x"] {
            assert!(!e.key_down(&text(c, 1)));
        }
        e.key_down(&KeyEvent { code: 1, mods: Mods::default(), kind: KeyKind::Backspace, target: 1, repeat: false });
        e.key_down(&text("o", 1));
        assert_eq!(buffer(&e), "akuo");
    }

    #[test]
    fn navigation_click_and_app_switch_reset() {
        let e = engine();
        e.key_down(&text("a", 1));
        e.key_down(&KeyEvent { code: 2, mods: Mods::default(), kind: KeyKind::Other, target: 1, repeat: false });
        assert_eq!(buffer(&e), "");
        e.key_down(&text("a", 1));
        e.mouse_down();
        assert_eq!(buffer(&e), "");
        e.key_down(&text("a", 1));
        e.key_down(&text("b", 2));
        assert_eq!(buffer(&e), "b");
    }

    #[test]
    fn hotkey_is_swallowed_and_not_buffered() {
        let e = engine();
        let code = platform::key_code("KeyO");
        if code.is_none() {
            return; // platform without a key table (unit tests on other OSes)
        }
        e.key_down(&text("a", 1));
        let hk = KeyEvent {
            code: code.unwrap(),
            mods: Mods { alt: true, ..Mods::default() },
            kind: KeyKind::Text("ø".into()),
            target: 1,
            repeat: false,
        };
        assert!(e.key_down(&hk));
        assert!(e.key_up(code.unwrap()));
        assert_eq!(buffer(&e), "a");
    }

    #[test]
    fn buffer_is_capped() {
        let e = engine();
        for _ in 0..(MAX_BUFFER + 50) {
            e.key_down(&text("a", 1));
        }
        assert_eq!(e.lock().buffer.len(), MAX_BUFFER);
    }
}
