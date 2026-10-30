//! Notification-area icon: status at a glance and one-click switching.

use budmix_core::{Command, ConnectionState, DeviceId, DeviceView, SourceView};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::labels::{menu_label, phone_menu_label, tooltip};
use crate::state::AppState;
use crate::system;

pub const TRAY_ID: &str = "main";
const DEVICE_PREFIX: &str = "device:";
const PHONE_PREFIX: &str = "phone:";
const AUTO_RECONNECT_ID: &str = "auto-reconnect";
const OPEN_ID: &str = "open";
const QUIT_ID: &str = "quit";

const ICON_IDLE: &[u8] = include_bytes!("../icons/tray-idle.png");
const ICON_CONNECTED: &[u8] = include_bytes!("../icons/tray-connected.png");

fn current_views(app: &AppHandle) -> (Vec<DeviceView>, Vec<SourceView>) {
    app.try_state::<AppState>()
        .map(|state| (state.runtime().views(), state.bridge().views()))
        .unwrap_or_default()
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let (devices, phones) = current_views(app);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON_IDLE)?)
        .tooltip(tooltip(&devices, &phones))
        .menu(&build_menu(app, &devices, &phones)?)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                crate::dock::toggle(tray.app_handle(), crate::dock::to_px(&rect, 1.0));
            }
        })
        .build(app)?;
    refresh(app)
}

/// Rebuilds the menu, tooltip and icon from the current state. Safe to call
/// from any thread.
pub fn update(app: &AppHandle) {
    let handle = app.clone();
    let scheduled = app.run_on_main_thread(move || {
        if let Err(err) = refresh(&handle) {
            log::warn!("tray update failed: {err}");
        }
    });
    if let Err(err) = scheduled {
        log::warn!("could not schedule tray update: {err}");
    }
}

fn refresh(app: &AppHandle) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let (devices, phones) = current_views(app);
    let connected = devices
        .iter()
        .any(|view| view.state == ConnectionState::Connected);
    tray.set_menu(Some(build_menu(app, &devices, &phones)?))?;
    tray.set_tooltip(Some(tooltip(&devices, &phones)))?;
    tray.set_icon(Some(Image::from_bytes(if connected {
        ICON_CONNECTED
    } else {
        ICON_IDLE
    })?))?;
    Ok(())
}

fn build_menu(
    app: &AppHandle,
    devices: &[DeviceView],
    phones: &[SourceView],
) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    if devices.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "none",
            "No paired Bluetooth audio devices",
            false,
            None::<&str>,
        )?)?;
    }
    for view in devices {
        menu.append(&CheckMenuItem::with_id(
            app,
            format!("{DEVICE_PREFIX}{}", view.id),
            menu_label(view),
            true,
            view.state == ConnectionState::Connected,
            None::<&str>,
        )?)?;
    }
    if !phones.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        for phone in phones {
            menu.append(&CheckMenuItem::with_id(
                app,
                format!("{PHONE_PREFIX}{}", phone.id),
                phone_menu_label(phone),
                true,
                phone.enabled,
                None::<&str>,
            )?)?;
        }
    }
    let auto_reconnect = app
        .try_state::<AppState>()
        .is_none_or(|state| state.settings().auto_reconnect);
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        AUTO_RECONNECT_ID,
        "Auto-reconnect",
        true,
        auto_reconnect,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        OPEN_ID,
        "Settings…",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        QUIT_ID,
        "Quit",
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let state = app.state::<AppState>();
    match event.id().as_ref() {
        OPEN_ID => crate::show_main_window(app),
        QUIT_ID => {
            state.prepare_quit();
            app.exit(0);
        }
        AUTO_RECONNECT_ID => {
            let mut settings = state.settings();
            settings.auto_reconnect = !settings.auto_reconnect;
            if let Err(err) = system::update_settings(app, settings) {
                log::warn!("{err}");
            }
        }
        id => {
            if let Some(device) = id.strip_prefix(DEVICE_PREFIX) {
                state.runtime().send(Command::Toggle(DeviceId::new(device)));
            } else if let Some(phone) = id.strip_prefix(PHONE_PREFIX) {
                let id = DeviceId::new(phone);
                let enabled = state
                    .bridge()
                    .views()
                    .iter()
                    .any(|view| view.id == id && view.enabled);
                if let Err(err) = system::set_phone_bridged(app, id, !enabled) {
                    log::warn!("{err}");
                }
            }
        }
    }
}
