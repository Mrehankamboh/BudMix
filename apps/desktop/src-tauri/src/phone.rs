//! Glue between the phone paths: the cast runtime (Wi-Fi), the bridge
//! runtime (Bluetooth) and the audio runtime, plus the notification.

use std::sync::{Mutex, PoisonError};

use budslink_core::{BridgeState, CastPhase, CastStatus, PhoneMode, SourceView};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::state::AppState;
use crate::{CAST_CHANGED, SOURCES_CHANGED, system, tray};

/// The Bluetooth phone list changed.
pub fn sources_changed(app: &AppHandle, views: &[SourceView]) {
    if let Err(err) = app.emit(SOURCES_CHANGED, views) {
        log::warn!("could not notify the UI: {err}");
    }
    tray::update(app);
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let playing = views
        .iter()
        .any(|view| view.state == BridgeState::Streaming);
    state.audio().set_bluetooth_playing(playing);
    seed_phones(app, &state, views);
}

/// First run: "BudsLink connects the paired phone itself over Bluetooth", so
/// every paired phone is turned on once. The user can turn any off later.
fn seed_phones(app: &AppHandle, state: &AppState, views: &[SourceView]) {
    let settings = state.settings();
    if settings.phones_seeded || views.is_empty() || settings.phone_mode == PhoneMode::WifiOnly {
        return;
    }
    state.update_owned(|settings| settings.phones_seeded = true);
    for view in views.iter().filter(|view| !view.enabled) {
        log::info!("turning on Bluetooth phone audio for {}", view.name);
        if let Err(err) = system::set_phone_bridged(app, view.id.clone(), true) {
            log::warn!("{err}");
        }
    }
}

/// The cast status changed: tell the UI, hold Bluetooth off while casting,
/// and say so when the phone's audio starts playing.
pub fn cast_changed(app: &AppHandle, status: &CastStatus, last_phase: &Mutex<CastPhase>) {
    if let Err(err) = app.emit(CAST_CHANGED, status) {
        log::warn!("could not notify the UI: {err}");
    }
    if let Some(state) = app.try_state::<AppState>() {
        state.bridge().set_suspended(status.bluetooth_suspended);
    }
    let mut last = last_phase.lock().unwrap_or_else(PoisonError::into_inner);
    let previous = std::mem::replace(&mut *last, status.cast.phase);
    drop(last);
    let phone = status.cast.phone.as_deref().unwrap_or("Phone");
    match (previous, status.cast.phase) {
        (from, CastPhase::Playing) if from != CastPhase::Playing => {
            notify(
                app,
                "Phone audio is now in your buds",
                &format!("{phone} is casting over Wi-Fi."),
            );
        }
        (from, CastPhase::NeedsApproval) if from != CastPhase::NeedsApproval => {
            notify(
                app,
                &format!("{phone} wants to cast"),
                "Open BudsLink from the tray to allow or refuse it.",
            );
            crate::dock::show(app);
        }
        _ => {}
    }
    tray::update(app);
}

/// The cast allowlist changed (a phone was allowed or forgotten).
pub fn allowed_changed(app: &AppHandle, allowed: Vec<String>) {
    if let Some(state) = app.try_state::<AppState>() {
        let settings = state.update_owned(|settings| settings.allowed_phones = allowed);
        let _ = app.emit(system::SETTINGS_CHANGED, &settings);
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        log::debug!("notification failed: {err}");
    }
}
