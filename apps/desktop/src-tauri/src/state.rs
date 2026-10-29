//! Application state shared by commands, the tray and the hotkey handler.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use budslink_core::{
    AudioHandle, AudioRuntime, BridgeHandle, BridgeRuntime, Runtime, RuntimeHandle, Settings,
    SourceBackend,
};
use serde::Serialize;

/// What the UI needs to know about the running app.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub backend: &'static str,
    pub version: String,
    pub mica: bool,
    pub warnings: Vec<String>,
}

pub struct AppState {
    runtime: Runtime,
    bridge: BridgeRuntime,
    audio: AudioRuntime,
    /// Also driven by `bridge`; kept for the phone list and diagnostics.
    sources: Arc<dyn SourceBackend>,
    settings: Mutex<Settings>,
    settings_path: PathBuf,
    warnings: Mutex<Vec<String>>,
    mica: bool,
    quitting: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AppState {
    pub fn new(
        runtime: Runtime,
        bridge: BridgeRuntime,
        audio: AudioRuntime,
        sources: Arc<dyn SourceBackend>,
        settings: Settings,
        settings_path: PathBuf,
        mica: bool,
    ) -> Self {
        Self {
            runtime,
            bridge,
            audio,
            sources,
            settings: Mutex::new(settings),
            settings_path,
            warnings: Mutex::new(Vec::new()),
            mica,
            quitting: AtomicBool::new(false),
        }
    }

    pub fn runtime(&self) -> RuntimeHandle {
        self.runtime.handle()
    }

    pub fn bridge(&self) -> BridgeHandle {
        self.bridge.handle()
    }

    pub fn audio(&self) -> AudioHandle {
        self.audio.handle()
    }

    pub fn sources(&self) -> Arc<dyn SourceBackend> {
        Arc::clone(&self.sources)
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    /// Persists new settings and hands them to the runtimes.
    pub fn store_settings(&self, settings: Settings) -> Result<(), String> {
        settings
            .save(&self.settings_path)
            .map_err(|err| format!("Could not save settings: {err}"))?;
        self.runtime
            .handle()
            .configure(settings.controller_config());
        let routing_changed = lock(&self.settings).route_laptop_audio != settings.route_laptop_audio;
        if routing_changed {
            self.audio.handle().set_routing(settings.route_laptop_audio);
        }
        *lock(&self.settings) = settings;
        Ok(())
    }

    /// Remembers the output to restore later, so it survives a restart.
    pub fn remember_saved_output(&self, saved: Option<String>) {
        let mut settings = lock(&self.settings);
        if settings.saved_output == saved {
            return;
        }
        settings.saved_output = saved;
        if let Err(err) = settings.save(&self.settings_path) {
            log::warn!("could not save the output to restore: {err}");
        }
    }

    pub fn set_warnings(&self, warnings: Vec<String>) {
        *lock(&self.warnings) = warnings;
    }

    pub fn info(&self, version: String) -> AppInfo {
        AppInfo {
            backend: self.runtime.handle().backend_name(),
            version,
            mica: self.mica,
            warnings: lock(&self.warnings).clone(),
        }
    }

    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }

    /// Marks the app as quitting and gives the previous default output back.
    pub fn prepare_quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        self.audio.handle().stop();
    }
}

/// Loads settings; a corrupt file is moved aside and defaults are used.
pub fn load_settings(path: &Path) -> Settings {
    match Settings::load(path) {
        Ok(settings) => settings,
        Err(err) => {
            log::warn!("{err}; starting with default settings");
            let backup = path.with_extension("invalid.json");
            if let Err(err) = fs::rename(path, &backup) {
                log::warn!("could not move the invalid settings file aside: {err}");
            }
            Settings::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_settings_are_moved_aside() {
        let dir = std::env::temp_dir().join(format!("budslink-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, "{ broken").unwrap();

        assert_eq!(load_settings(&path), Settings::default());
        assert!(!path.exists());
        assert!(dir.join("settings.invalid.json").exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
