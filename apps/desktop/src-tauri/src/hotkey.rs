//! Global toggle hotkey.

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::state::AppState;

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if let Some(state) = app.try_state::<AppState>() {
                state
                    .runtime()
                    .toggle_target(state.settings().preferred_device);
            }
        })
        .build()
}

/// Replaces the registered hotkey. `None` just clears it.
pub fn register(app: &AppHandle, hotkey: Option<&str>) -> Result<(), String> {
    let shortcuts = app.global_shortcut();
    shortcuts
        .unregister_all()
        .map_err(|err| format!("Could not clear the previous hotkey: {err}"))?;
    let Some(hotkey) = hotkey else {
        return Ok(());
    };
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|err| format!("\"{hotkey}\" is not a valid hotkey ({err})."))?;
    shortcuts
        .register(shortcut)
        .map_err(|err| format!("Could not register {hotkey}; another app may be using it ({err})."))
}
