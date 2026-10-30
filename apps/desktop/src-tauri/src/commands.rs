//! Commands invoked by the web UI.

use budmix_core::{
    AudioView, CastStatus, Command, DeviceId, DeviceView, Level, Settings, SourceView,
    UnavailableSource, VolumeSource,
};
use budmix_win::checks::{CheckResult, Fix};
use tauri::{AppHandle, Manager, State};

use crate::state::{AppInfo, AppState};
use crate::{diagnostics, setup, system};

#[tauri::command]
pub fn app_info(app: AppHandle, state: State<'_, AppState>) -> AppInfo {
    state.info(app.package_info().version.to_string())
}

#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> Vec<DeviceView> {
    state.runtime().views()
}

/// Requests are asynchronous: progress arrives through `devices-changed`.
#[tauri::command]
pub fn connect_device(state: State<'_, AppState>, id: String) {
    state.runtime().send(Command::Connect(DeviceId::new(id)));
}

#[tauri::command]
pub fn disconnect_device(state: State<'_, AppState>, id: String) {
    state.runtime().send(Command::Disconnect(DeviceId::new(id)));
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: Settings) -> Result<AppInfo, String> {
    system::update_settings(&app, settings)
}

#[tauri::command]
pub fn list_sources(state: State<'_, AppState>) -> Vec<SourceView> {
    state.bridge().views()
}

/// Turns bridge mode on or off for one phone; progress arrives through
/// `sources-changed`.
#[tauri::command]
pub fn set_source_enabled(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    system::set_phone_bridged(&app, DeviceId::new(id), enabled)
}

/// Paired phones Windows won't stream audio from, with the reason.
#[tauri::command]
pub async fn list_unavailable_phones(
    state: State<'_, AppState>,
) -> Result<Vec<UnavailableSource>, String> {
    let sources = state.sources();
    tauri::async_runtime::spawn_blocking(move || sources.unavailable())
        .await
        .map_err(|err| err.to_string())
}

/// What BudMix shows and Windows reports, as text for a bug report.
#[tauri::command]
pub async fn diagnostics(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let sources = state.sources();
    let probe = app.clone();
    let system = tauri::async_runtime::spawn_blocking(move || {
        format!(
            "{}\n\n{}",
            setup::diagnostics(&probe),
            sources.diagnostics()
        )
    })
    .await
    .map_err(|err| err.to_string())?;
    let cast = state.cast().status().map_or_else(
        || "Phone over Wi-Fi: not running".to_owned(),
        |status| {
            format!(
                "Phone over Wi-Fi: receiver {:?} · {:?} · phone {} · mode {:?} · Bluetooth held off: {}{}",
                status.cast.receiver,
                status.cast.phase,
                status.cast.phone.as_deref().unwrap_or("none"),
                status.mode,
                status.bluetooth_suspended,
                status
                    .cast
                    .message
                    .as_deref()
                    .map(|message| format!(" · {message}"))
                    .unwrap_or_default()
            )
        },
    );
    let system = format!("{cast}\n{system}");
    let version = app.package_info().version.to_string();
    let header = diagnostics::Header {
        version: &version,
        backend: state.runtime().backend_name(),
        windows_build: crate::windows_build(),
    };
    Ok(diagnostics::render(
        &header,
        &state.runtime().views(),
        &state.bridge().views(),
        &state.audio().view(),
        &state.info(version.clone()).warnings,
        &system,
    ))
}

/// Laptop audio: default output, buds output, call mode.
#[tauri::command]
pub fn audio_state(state: State<'_, AppState>) -> AudioView {
    state.audio().view()
}

/// Plays the test beep into the buds (or the default output) and returns
/// once it has played.
#[tauri::command]
pub async fn play_test_tone(state: State<'_, AppState>) -> Result<(), String> {
    let audio = state.audio();
    tauri::async_runtime::spawn_blocking(move || audio.play_tone())
        .await
        .map_err(|err| err.to_string())?
}

/// Opens the full window (settings) and closes the dock.
#[tauri::command]
pub fn open_settings(app: AppHandle) {
    crate::show_main_window(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle, state: State<'_, AppState>) {
    state.prepare_quit();
    app.exit(0);
}

/// The dock page reports its content height so the window fits it.
#[tauri::command]
pub fn dock_resize(app: AppHandle, height: f64) -> Result<(), String> {
    crate::dock::resize(&app, height).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn hide_dock(app: AppHandle) {
    crate::dock::hide(&app);
}

/// The setup check: every prerequisite with its status and fix.
#[tauri::command]
pub async fn setup_checks(app: AppHandle) -> Result<Vec<CheckResult>, String> {
    tauri::async_runtime::spawn_blocking(move || setup::run_checks(&app))
        .await
        .map_err(|err| err.to_string())
}

/// Runs the fix behind a check's "Fix" button.
#[tauri::command]
pub async fn run_fix(app: AppHandle, fix: Fix) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || setup::run_fix(&app, fix))
        .await
        .map_err(|err| err.to_string())?
}

/// Phone audio over Wi-Fi: receiver, casting phone, approvals.
#[tauri::command]
pub fn cast_state(state: State<'_, AppState>) -> Option<CastStatus> {
    state.cast().status()
}

/// Lets the phone that waits for approval play; `remember` allows it for good.
#[tauri::command]
pub fn approve_phone(state: State<'_, AppState>, remember: bool) {
    state.cast().approve(remember);
}

#[tauri::command]
pub fn reject_phone(state: State<'_, AppState>) {
    state.cast().reject();
}

/// Removes a phone from the cast allowlist; it will be asked about again.
#[tauri::command]
pub fn forget_phone(state: State<'_, AppState>, name: String) {
    state.cast().forget(name);
}

/// Moves one volume slider (or mutes it) and remembers it.
#[tauri::command]
pub fn set_volume(app: AppHandle, source: VolumeSource, level: Level) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut settings = state.settings();
    settings.volumes.set(source, level);
    system::update_settings(&app, settings).map(|_| ())
}

/// Shows or hides the phone's picture; the audio plays either way.
#[tauri::command]
pub fn show_cast_video(app: AppHandle, visible: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let settings = Settings {
        show_cast_video: visible,
        ..state.settings()
    };
    system::update_settings(&app, settings).map(|_| ())
}
