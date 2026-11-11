//! Menu-bar (macOS) / system-tray (Windows) icon and its menu.

use crate::settings::hotkey_label;
use crate::{open_window, set_enabled, AppState, Status};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

const TRAY_ID: &str = "oops";

#[cfg(target_os = "macos")]
mod icons {
    pub const NORMAL: &[u8] = include_bytes!("../tray/mac.png");
    pub const PAUSED: &[u8] = include_bytes!("../tray/mac-paused.png");
    pub const ALERT: &[u8] = include_bytes!("../tray/mac-alert.png");
    pub const OK: &[u8] = include_bytes!("../tray/mac-ok.png");
}
#[cfg(not(target_os = "macos"))]
mod icons {
    pub const NORMAL: &[u8] = include_bytes!("../tray/win.png");
    pub const PAUSED: &[u8] = include_bytes!("../tray/win-paused.png");
    pub const ALERT: &[u8] = include_bytes!("../tray/win-alert.png");
    pub const OK: &[u8] = include_bytes!("../tray/win-ok.png");
}

struct Text {
    active: &'static str,
    paused: &'static str,
    needs_permission: &'static str,
    error: &'static str,
    pause: &'static str,
    resume: &'static str,
    settings: &'static str,
    update: &'static str,
    quit: &'static str,
}

const HE: Text = Text {
    active: "Oops פועל · {hk} לתיקון",
    paused: "Oops מושהה",
    needs_permission: "⚠️ חסרה הרשאה, לחצו לסידור",
    error: "⚠️ משהו לא עובד, לחצו לפרטים",
    pause: "השהיה",
    resume: "הפעלה מחדש",
    settings: "הגדרות…",
    update: "גרסה חדשה זמינה ({v})…",
    quit: "יציאה מ־Oops",
};

const EN: Text = Text {
    active: "Oops is on · {hk} to fix",
    paused: "Oops is paused",
    needs_permission: "⚠️ Needs permission, click to fix",
    error: "⚠️ Something's wrong, click for details",
    pause: "Pause Oops",
    resume: "Resume Oops",
    settings: "Settings…",
    update: "Update available ({v})…",
    quit: "Quit Oops",
};

fn icon(bytes: &'static [u8]) -> Option<Image<'static>> {
    Image::from_bytes(bytes).ok()
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let t = if settings.is_hebrew() { &HE } else { &EN };
    let status = state.status();
    let (label, clickable) = match status {
        Status::Active => (t.active.replace("{hk}", &hotkey_label(&settings.hotkey)), false),
        Status::Paused => (t.paused.to_string(), false),
        Status::NeedsPermission => (t.needs_permission.to_string(), true),
        Status::Error => (t.error.to_string(), true),
    };
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "status", label, clickable, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    let toggle = if settings.enabled { t.pause } else { t.resume };
    menu.append(&MenuItem::with_id(app, "toggle", toggle, true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "settings", t.settings, true, None::<&str>)?)?;
    if let Some(update) = state.update() {
        menu.append(&MenuItem::with_id(app, "update", t.update.replace("{v}", &update.version), true, None::<&str>)?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", t.quit, true, None::<&str>)?)?;
    Ok(menu)
}

fn status_icon(status: Status) -> &'static [u8] {
    match status {
        Status::Active => icons::NORMAL,
        Status::Paused => icons::PAUSED,
        Status::NeedsPermission | Status::Error => icons::ALERT,
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let status = app.state::<AppState>().status();
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Oops")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .icon_as_template(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            "status" | "settings" => open_window(app),
            "toggle" => {
                let enabled = app.state::<AppState>().settings().enabled;
                set_enabled(app, !enabled);
            }
            "update" => {
                if let Some(update) = app.state::<AppState>().update() {
                    crate::open_external(app, &update.url);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Windows convention: left click opens the app, right click shows the menu.
            if cfg!(not(target_os = "macos")) {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                    open_window(tray.app_handle());
                }
            }
        });
    if let Some(img) = icon(status_icon(status)) {
        builder = builder.icon(img);
    }
    builder.build(app)?;
    Ok(())
}

/// Rebuilds the menu and icon from the current state.
pub fn refresh(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = app2.tray_by_id(TRAY_ID) {
            if let Ok(menu) = build_menu(&app2) {
                let _ = tray.set_menu(Some(menu));
            }
            let status = app2.state::<AppState>().status();
            let _ = tray.set_icon(icon(status_icon(status)));
            let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
        }
    });
}

/// Briefly shows a check mark after a successful fix.
pub fn flash(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(tray) = app2.tray_by_id(TRAY_ID) {
            let _ = tray.set_icon(icon(icons::OK));
            let _ = tray.set_icon_as_template(cfg!(target_os = "macos"));
        }
    });
    let app3 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(900));
        refresh(&app3);
    });
}
