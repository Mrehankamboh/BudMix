//! Applies settings that live outside BudsLink: hotkey and autostart.

use budslink_core::{DeviceId, Settings};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::state::{AppInfo, AppState};
use crate::{hotkey, tray};

pub const SETTINGS_CHANGED: &str = "settings-changed";

/// Registers the hotkey and syncs "Start with Windows"; returns warnings.
pub fn apply(app: &AppHandle, settings: &Settings) -> Vec<String> {
    let mut warnings = Vec::new();
    if let Err(warning) = hotkey::register(app, settings.toggle_hotkey.as_deref()) {
        warnings.push(warning);
    }
    if let Err(warning) = sync_autostart(app, settings.start_with_windows) {
        warnings.push(warning);
    }
    for warning in &warnings {
        log::warn!("{warning}");
    }
    warnings
}

fn sync_autostart(app: &AppHandle, wanted: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if wanted {
        // Always (re)write it, so it points at this copy of BudsLink even
        // if an older copy elsewhere registered itself first.
        launcher.enable()
    } else if launcher.is_enabled().unwrap_or(true) {
        launcher.disable()
    } else {
        Ok(())
    };
    result.map_err(|err| format!("Could not update Start with Windows: {err}"))
}

/// Saves new settings from the UI or tray and applies them everywhere.
pub fn update_settings(app: &AppHandle, settings: Settings) -> Result<AppInfo, String> {
    let state = app.state::<AppState>();
    // The output to restore belongs to the audio runtime, not the UI.
    let settings = Settings {
        saved_output: state.settings().saved_output,
        ..settings
    }
    .sanitized();
    state.store_settings(settings.clone())?;
    state.set_warnings(apply(app, &settings));
    tray::update(app);
    let _ = app.emit(SETTINGS_CHANGED, &settings);
    Ok(state.info(app.package_info().version.to_string()))
}

/// Turns bridge mode on or off for a phone and remembers it for next start.
pub fn set_phone_bridged(app: &AppHandle, id: DeviceId, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    state.bridge().set_enabled(id.clone(), enabled);
    let mut settings = state.settings();
    settings.bridged_phones.retain(|phone| phone != &id);
    if enabled {
        settings.bridged_phones.push(id);
    }
    let settings = settings.sanitized();
    state.store_settings(settings.clone())?;
    let _ = app.emit(SETTINGS_CHANGED, &settings);
    Ok(())
}
