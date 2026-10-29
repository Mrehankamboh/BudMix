//! BudsLink desktop shell: window, tray, hotkey and autostart around the
//! platform-independent runtimes in `budslink-core` (buds switching and
//! bridge mode).

mod commands;
mod diagnostics;
mod dock;
mod hotkey;
mod instance;
mod labels;
mod state;
mod system;
mod tray;

use std::sync::Arc;

use budslink_core::bridge::default_retry_policy;
use budslink_core::{
    AudioBackend, AudioRuntime, BridgeRuntime, DeviceBackend, DeviceView, Runtime, SourceBackend,
    route_target,
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::state::AppState;

pub const MAIN_WINDOW: &str = "main";
/// Passed by autostart so BudsLink starts in the tray.
pub const MINIMIZED_ARG: &str = "--minimized";
pub const DEVICES_CHANGED: &str = "devices-changed";
pub const SOURCES_CHANGED: &str = "sources-changed";
pub const AUDIO_CHANGED: &str = "audio-changed";
/// Set to `1` to use simulated devices, e.g. for demos and screenshots.
pub const DEMO_ENV: &str = "BUDSLINK_DEMO";

pub fn run() {
    instance::wait_if_replacing();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            instance::second_instance(app, &args, &cwd)
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![MINIMIZED_ARG]),
        ))
        .plugin(hotkey::plugin())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_devices,
            commands::connect_device,
            commands::disconnect_device,
            commands::get_settings,
            commands::update_settings,
            commands::list_sources,
            commands::set_source_enabled,
            commands::list_unavailable_phones,
            commands::diagnostics,
            commands::open_settings,
            commands::quit_app,
            commands::dock_resize,
            commands::hide_dock,
            commands::audio_state,
            commands::play_test_tone,
        ])
        .setup(|app| {
            setup(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                // Closing a window keeps BudsLink running in the tray.
                let quitting = window
                    .app_handle()
                    .try_state::<AppState>()
                    .is_some_and(|state| state.is_quitting());
                if !quitting {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            WindowEvent::Focused(false) if window.label() == dock::DOCK_WINDOW => {
                dock::focus_lost(window.app_handle());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running BudsLink");
}

fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let settings_path = app.path().app_config_dir()?.join("settings.json");
    let settings = state::load_settings(&settings_path);
    let (backend, sources, audio_backend) = create_backends();
    log::info!(
        "BudsLink {} starting with the {} backend",
        app.package_info().version,
        backend.name()
    );

    let events = app.clone();
    let runtime = Runtime::spawn(
        backend,
        settings.controller_config(),
        Box::new(move |views| {
            if let Err(err) = events.emit(DEVICES_CHANGED, views) {
                log::warn!("could not notify the UI: {err}");
            }
            route_laptop_audio(&events, views);
            tray::update(&events);
        }),
    );

    let events = app.clone();
    let audio = AudioRuntime::spawn(
        audio_backend,
        settings.route_laptop_audio,
        settings.saved_output.clone(),
        Box::new(move |view| {
            if let Some(state) = events.try_state::<AppState>() {
                state.remember_saved_output(view.saved_output.clone());
            }
            if let Err(err) = events.emit(AUDIO_CHANGED, view) {
                log::warn!("could not notify the UI: {err}");
            }
        }),
    );

    let events = app.clone();
    let bridge = BridgeRuntime::spawn(
        Arc::clone(&sources),
        settings.bridged_phones.clone(),
        default_retry_policy(),
        Box::new(move |views| {
            if let Err(err) = events.emit(SOURCES_CHANGED, views) {
                log::warn!("could not notify the UI: {err}");
            }
            tray::update(&events);
        }),
    );

    let mica = supports_mica();
    app.manage(AppState::new(
        runtime,
        bridge,
        audio,
        sources,
        settings.clone(),
        settings_path,
        mica,
    ));
    let state = app.state::<AppState>();
    state.set_warnings(system::apply(app, &settings));
    // Device updates published before the state existed were not routed.
    route_laptop_audio(app, &state.runtime().views());
    tray::create(app)?;
    create_main_window(app, mica)?;
    dock::create(app, mica)?;
    // Everyday control lives in the dock; the full window is for settings.
    if !std::env::args().any(|arg| arg == MINIMIZED_ARG) {
        dock::show(app);
    }
    Ok(())
}

/// Tells the audio runtime which buds laptop audio should go to.
fn route_laptop_audio(app: &AppHandle, views: &[DeviceView]) {
    if let Some(state) = app.try_state::<AppState>() {
        let preferred = state.settings().preferred_device;
        state.audio().set_buds(route_target(views, preferred.as_ref()));
    }
}

type Backends = (
    Arc<dyn DeviceBackend>,
    Arc<dyn SourceBackend>,
    Arc<dyn AudioBackend>,
);

fn create_backends() -> Backends {
    let demo = std::env::var(DEMO_ENV).is_ok_and(|value| value == "1");
    #[cfg(windows)]
    if !demo {
        return (
            Arc::new(budslink_win::WindowsBackend::new()),
            Arc::new(budslink_win::PhoneAudioBackend::new()),
            Arc::new(budslink_win::WindowsAudioBackend::new()),
        );
    }
    if !demo {
        log::warn!("no Bluetooth backend for this platform; using demo devices");
    }
    let devices = budslink_core::fake::FakeBackend::demo();
    (
        Arc::new(devices.clone()),
        Arc::new(budslink_core::fake::FakeSourceBackend::demo()),
        Arc::new(budslink_core::fake::FakeAudioBackend::linked(devices)),
    )
}

fn supports_mica() -> bool {
    #[cfg(windows)]
    return budslink_win::supports_mica();
    #[cfg(not(windows))]
    false
}

pub fn windows_build() -> Option<u32> {
    #[cfg(windows)]
    return budslink_win::windows_build();
    #[cfg(not(windows))]
    None
}

/// The full window, used for settings. It starts hidden.
fn create_main_window(app: &AppHandle, mica: bool) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::default())
        .title("BudsLink settings")
        .inner_size(460.0, 720.0)
        .min_inner_size(380.0, 520.0)
        .center()
        .visible(false);
    let builder = if mica {
        use tauri::window::{Effect, EffectsBuilder};
        builder
            .transparent(true)
            .effects(EffectsBuilder::new().effect(Effect::Mica).build())
    } else {
        builder
    };
    builder.build()?;
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    dock::hide(app);
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}
