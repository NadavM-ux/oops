// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod tray;
mod updates;

use oops_core::system::{self, Permission, Sound};
use oops_core::{convert, hotkey, Engine, EngineEvent, Hotkey, HotkeyError};
use serde::Serialize;
use settings::{Settings, SettingsPatch};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_opener::OpenerExt;

const BUNDLE_ID: &str = "tech.nadav.oops";
const AUTOSTART_FLAG: &str = "--autostart";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Active,
    Paused,
    NeedsPermission,
    Error,
}

pub struct AppState {
    settings: Mutex<Settings>,
    config_dir: PathBuf,
    engine: Engine,
    update: Mutex<Option<updates::Update>>,
    autostarted: bool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    pub fn update(&self) -> Option<updates::Update> {
        lock(&self.update).clone()
    }

    pub fn status(&self) -> Status {
        if system::permission() == Permission::Missing {
            Status::NeedsPermission
        } else if !lock(&self.settings).enabled {
            Status::Paused
        } else if !self.engine.is_running() {
            Status::Error
        } else {
            Status::Active
        }
    }

    fn modify(&self, f: impl FnOnce(&mut Settings)) -> Settings {
        let mut s = lock(&self.settings);
        f(&mut s);
        s.save(&self.config_dir);
        s.clone()
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateDto {
    settings: Settings,
    platform: &'static str,
    version: String,
    status: Status,
    permission: Permission,
    update: Option<updates::Update>,
    hebrew_layout: bool,
    english_layout: bool,
    translocated: bool,
    autostarted: bool,
}

fn snapshot(app: &AppHandle) -> StateDto {
    let state = app.state::<AppState>();
    StateDto {
        settings: state.settings(),
        platform: if cfg!(target_os = "macos") { "macos" } else { "windows" },
        version: app.package_info().version.to_string(),
        status: state.status(),
        permission: system::permission(),
        update: state.update(),
        hebrew_layout: system::has_layout(convert::Lang::Hebrew),
        english_layout: system::has_layout(convert::Lang::English),
        translocated: is_translocated(),
        autostarted: state.autostarted,
    }
}

/// Tells the settings window (if open) and the tray that something changed.
fn broadcast(app: &AppHandle) {
    tray::refresh(app);
    let _ = app.emit("oops://state", snapshot(app));
}

/// macOS runs apps opened straight from a downloaded disk image from a random
/// read-only location; permissions and "open at login" don't stick there.
fn is_translocated() -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    std::env::current_exe()
        .map(|p| {
            let p = p.to_string_lossy();
            p.contains("/AppTranslocation/") || p.starts_with("/Volumes/")
        })
        .unwrap_or(false)
}

pub fn open_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);

    let builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Oops")
        .inner_size(780.0, 660.0)
        .min_inner_size(660.0, 580.0)
        .center()
        .visible(false);
    #[cfg(target_os = "macos")]
    let builder = builder.title_bar_style(tauri::TitleBarStyle::Overlay).hidden_title(true);
    if let Ok(window) = builder.build() {
        // The page calls `ui_ready` once painted; this is only a safety net.
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            if !window.is_visible().unwrap_or(true) {
                let _ = window.show();
                let _ = window.set_focus();
            }
        });
    }
}

pub fn open_external(app: &AppHandle, url: &str) {
    if url.starts_with("https://") {
        let _ = app.opener().open_url(url, None::<&str>);
    }
}

pub fn set_enabled(app: &AppHandle, enabled: bool) {
    let state = app.state::<AppState>();
    let s = state.modify(|s| s.enabled = enabled);
    state.engine.set_config(&s.engine_config());
    broadcast(app);
}

fn apply_autostart(app: &AppHandle, on: bool) {
    let launcher = app.autolaunch();
    let current = launcher.is_enabled().unwrap_or(false);
    if on && (!current || cfg!(target_os = "macos")) {
        // Re-enabling on macOS refreshes the app path stored in the launch agent.
        let _ = launcher.enable();
    } else if !on && current {
        let _ = launcher.disable();
    }
}

// ---------------------------------------------------------------------------------------
// Commands called by the settings window
// ---------------------------------------------------------------------------------------

#[tauri::command]
fn get_state(app: AppHandle) -> StateDto {
    snapshot(&app)
}

#[tauri::command]
fn ui_ready(window: WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

#[tauri::command]
fn validate_hotkey(hotkey: Hotkey) -> Result<(), HotkeyError> {
    hotkey::validate(&hotkey, hotkey::CURRENT_OS)
}

#[tauri::command]
fn update_settings(app: AppHandle, patch: SettingsPatch) -> Result<StateDto, HotkeyError> {
    if let Some(hk) = &patch.hotkey {
        hotkey::validate(hk, hotkey::CURRENT_OS)?;
    }
    let state = app.state::<AppState>();
    let autostart = patch.launch_at_login;
    let s = state.modify(|s| {
        if let Some(v) = patch.hotkey {
            s.hotkey = v;
        }
        if let Some(v) = patch.enabled {
            s.enabled = v;
        }
        if let Some(v) = patch.switch_layout {
            s.switch_layout = v;
        }
        if let Some(v) = patch.sounds {
            s.sounds = v;
        }
        if let Some(v) = patch.launch_at_login {
            s.launch_at_login = v;
        }
        if let Some(v) = patch.check_updates {
            s.check_updates = v;
        }
        if let Some(v) = patch.language.filter(|l| l == "he" || l == "en") {
            s.language = v;
        }
    });
    state.engine.set_config(&s.engine_config());
    if let Some(on) = autostart {
        apply_autostart(&app, on);
    }
    broadcast(&app);
    Ok(snapshot(&app))
}

#[tauri::command]
fn set_recording(app: AppHandle, on: bool) {
    app.state::<AppState>().engine.set_recording(on);
}

#[tauri::command]
fn request_permission() {
    system::request_permission();
    system::open_permission_settings();
}

#[tauri::command]
fn finish_onboarding(app: AppHandle, launch_at_login: bool) -> StateDto {
    let state = app.state::<AppState>();
    state.modify(|s| {
        s.onboarded = true;
        s.launch_at_login = launch_at_login;
    });
    apply_autostart(&app, launch_at_login);
    broadcast(&app);
    // Setup is over: Oops now lives in the menu bar / tray, so the window goes away.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        if let Some(window) = handle.get_webview_window("main") {
            let _ = window.close();
        }
    });
    snapshot(&app)
}

#[tauri::command]
fn restart_onboarding(app: AppHandle) -> StateDto {
    app.state::<AppState>().modify(|s| s.onboarded = false);
    snapshot(&app)
}

/// Used by the "Fix" button in the practice box (the shortcut itself goes through the engine).
#[tauri::command]
fn convert_text(text: String) -> Option<String> {
    convert::plan_selection(&text).map(|(out, _)| out)
}

#[tauri::command]
fn open_url(app: AppHandle, url: String) {
    open_external(&app, &url);
}

#[tauri::command]
fn show_applications_folder() {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open").arg("/Applications").spawn();
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------------------------------
// Engine events & background loops
// ---------------------------------------------------------------------------------------

fn on_engine_event(app: &AppHandle, event: EngineEvent) {
    let state = app.state::<AppState>();
    let sounds = state.settings().sounds;
    match &event {
        EngineEvent::Fixed { undo, .. } => {
            if sounds {
                system::play_sound(Sound::Fixed);
            }
            if !undo {
                state.modify(|s| s.fix_count += 1);
            }
            tray::flash(app);
        }
        EngineEvent::NothingToFix => {
            if sounds {
                system::play_sound(Sound::Nothing);
            }
        }
    }
    let _ = app.emit("oops://engine", &event);
    let _ = app.emit("oops://state", snapshot(app));
}

/// Starts the keyboard hook as soon as it can run and keeps the UI in sync
/// (e.g. the user grants the macOS permission while the window is open).
fn watch_status(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Option<(Status, Permission)> = None;
        loop {
            let state = app.state::<AppState>();
            let permission = system::permission();
            if permission != Permission::Missing && !state.engine.is_running() {
                let _ = state.engine.start();
            }
            if permission == Permission::Granted && !state.settings().had_permission {
                state.modify(|s| s.had_permission = true);
            }
            let now = (state.status(), permission);
            if last != Some(now) {
                last = Some(now);
                broadcast(&app);
            }
            std::thread::sleep(Duration::from_millis(if now.0 == Status::Active { 3000 } else { 1000 }));
        }
    });
}

fn watch_updates(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        loop {
            let state = app.state::<AppState>();
            if state.settings().check_updates {
                let current = app.package_info().version.to_string();
                if let Some(update) = updates::check(&current) {
                    *lock(&state.update) = Some(update);
                    broadcast(&app);
                }
            }
            std::thread::sleep(Duration::from_secs(12 * 60 * 60));
        }
    });
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| open_window(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![AUTOSTART_FLAG])))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_state,
            ui_ready,
            validate_hotkey,
            update_settings,
            set_recording,
            request_permission,
            finish_onboarding,
            restart_onboarding,
            convert_text,
            open_url,
            show_applications_folder,
            quit
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let config_dir = app.path().app_config_dir()?;
            let mut settings = Settings::load(&config_dir);

            // An update changes the app's signature, which silently voids the macOS
            // permission. Clear the stale entry so the user can just switch it on again.
            if cfg!(target_os = "macos") && settings.had_permission && system::permission() == Permission::Missing {
                system::reset_permission(BUNDLE_ID);
                settings.had_permission = false;
                settings.save(&config_dir);
            }

            let handle = app.handle().clone();
            let engine = Engine::new(settings.engine_config(), move |event| on_engine_event(&handle, event));
            let _ = engine.start();

            let autostarted = std::env::args().any(|a| a == AUTOSTART_FLAG);
            let onboarded = settings.onboarded;
            let launch_at_login = settings.launch_at_login;
            app.manage(AppState {
                settings: Mutex::new(settings),
                config_dir,
                engine,
                update: Mutex::new(None),
                autostarted,
            });

            tray::create(app.handle())?;
            if onboarded && launch_at_login {
                apply_autostart(app.handle(), true);
            }
            watch_status(app.handle().clone());
            watch_updates(app.handle().clone());

            let needs_attention = system::permission() == Permission::Missing;
            if !onboarded || !autostarted || needs_attention {
                open_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    let app = window.app_handle();
                    app.state::<AppState>().engine.set_recording(false);
                    #[cfg(target_os = "macos")]
                    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Oops");

    app.run(|app, event| match event {
        // Closing the settings window must not quit: Oops keeps running in the tray.
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => open_window(app),
        _ => {
            let _ = app;
        }
    });
}
