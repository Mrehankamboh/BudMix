//! User settings, persisted as JSON.

use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;
use std::{fs, io};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::controller::ControllerConfig;
use crate::device::DeviceId;
use crate::phone_router::PhoneMode;
use crate::reconnect::ReconnectPolicy;
use crate::volume::Volumes;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("could not read or write settings: {0}")]
    Io(#[from] io::Error),
    #[error("settings file is not valid JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub version: u32,
    pub auto_reconnect: bool,
    pub reconnect: ReconnectPolicy,
    pub connect_timeout_ms: u64,
    /// Device the toggle hotkey acts on. `None` picks one automatically.
    pub preferred_device: Option<DeviceId>,
    /// Global shortcut, e.g. `"Ctrl+Alt+B"`. `None` disables it.
    pub toggle_hotkey: Option<String>,
    pub start_with_windows: bool,
    /// Phones whose audio is bridged through the laptop (bridge mode).
    pub bridged_phones: Vec<DeviceId>,
    /// Make the connected buds the default output, so all laptop audio
    /// plays in them.
    pub route_laptop_audio: bool,
    /// The default output before BudMix switched to the buds, restored
    /// later (kept here so it survives a crash or restart).
    pub saved_output: Option<String>,
    /// The first-run setup check was finished (or skipped) once.
    pub setup_done: bool,
    /// Where phone audio comes from: Auto (Wi-Fi while casting, otherwise
    /// Bluetooth), Wi-Fi only or Bluetooth only.
    pub phone_mode: PhoneMode,
    /// Phones allowed to cast without asking (their cast names).
    pub allowed_phones: Vec<String>,
    pub volumes: Volumes,
    /// Show the phone's screen in a small window while it casts.
    pub show_cast_video: bool,
    /// Paired phones were turned on for Bluetooth audio once, automatically.
    pub phones_seeded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            auto_reconnect: true,
            reconnect: ReconnectPolicy::default(),
            connect_timeout_ms: 15_000,
            preferred_device: None,
            toggle_hotkey: Some("Ctrl+Alt+B".to_owned()),
            start_with_windows: false,
            bridged_phones: Vec::new(),
            route_laptop_audio: true,
            saved_output: None,
            setup_done: false,
            phone_mode: PhoneMode::Auto,
            allowed_phones: Vec::new(),
            volumes: Volumes::default(),
            show_cast_video: true,
            phones_seeded: false,
        }
    }
}

impl Settings {
    pub const CURRENT_VERSION: u32 = 1;
    pub const MIN_CONNECT_TIMEOUT_MS: u64 = 2_000;
    pub const MAX_CONNECT_TIMEOUT_MS: u64 = 120_000;

    /// Loads settings, falling back to defaults when the file does not exist yet.
    pub fn load(path: &Path) -> Result<Self, SettingsError> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(serde_json::from_str::<Self>(&text)?.sanitized()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err.into()),
        }
    }

    /// Writes settings atomically (temp file + rename).
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Clamps values into supported ranges and normalises empty strings.
    pub fn sanitized(mut self) -> Self {
        self.version = Self::CURRENT_VERSION;
        self.reconnect = self.reconnect.sanitized();
        self.connect_timeout_ms = self
            .connect_timeout_ms
            .clamp(Self::MIN_CONNECT_TIMEOUT_MS, Self::MAX_CONNECT_TIMEOUT_MS);
        self.toggle_hotkey = self
            .toggle_hotkey
            .map(|hotkey| hotkey.trim().to_owned())
            .filter(|hotkey| !hotkey.is_empty());
        self.preferred_device = self
            .preferred_device
            .filter(|id| !id.as_str().trim().is_empty());
        self.bridged_phones
            .retain(|id| !id.as_str().trim().is_empty());
        self.bridged_phones.sort();
        self.bridged_phones.dedup();
        self.saved_output = self.saved_output.filter(|id| !id.trim().is_empty());
        self.allowed_phones = self
            .allowed_phones
            .into_iter()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .collect();
        self.allowed_phones.sort_by_key(|name| name.to_lowercase());
        self.allowed_phones
            .dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        self.volumes = self.volumes.sanitized();
        self
    }

    pub fn controller_config(&self) -> ControllerConfig {
        ControllerConfig {
            auto_reconnect: self.auto_reconnect,
            reconnect: self.reconnect,
            connect_timeout: Duration::from_millis(self.connect_timeout_ms),
            ..ControllerConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::load(&dir.path().join("settings.json")).unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let settings = Settings {
            auto_reconnect: false,
            preferred_device: Some(DeviceId::new("{1234}")),
            toggle_hotkey: Some("Ctrl+Shift+F9".into()),
            start_with_windows: true,
            bridged_phones: vec![DeviceId::new("phone-a")],
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, r#"{ "autoReconnect": false, "someFutureField": 1 }"#).unwrap();
        let settings = Settings::load(&path).unwrap();
        assert!(!settings.auto_reconnect);
        assert_eq!(settings.toggle_hotkey.as_deref(), Some("Ctrl+Alt+B"));
        // Files written before laptop-audio routing existed turn it on.
        assert!(settings.route_laptop_audio);
        assert_eq!(settings.saved_output, None);
    }

    #[test]
    fn blank_saved_output_is_dropped() {
        let settings = Settings {
            saved_output: Some("  ".into()),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(settings.saved_output, None);
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        assert!(matches!(
            Settings::load(&path),
            Err(SettingsError::Parse(_))
        ));
    }

    #[test]
    fn sanitized_clamps_and_normalises() {
        let settings = Settings {
            version: 0,
            connect_timeout_ms: 1,
            toggle_hotkey: Some("   ".into()),
            preferred_device: Some(DeviceId::new(" ")),
            bridged_phones: vec![
                DeviceId::new("b"),
                DeviceId::new(""),
                DeviceId::new("a"),
                DeviceId::new("b"),
            ],
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(
            settings.bridged_phones,
            vec![DeviceId::new("a"), DeviceId::new("b")]
        );
        assert_eq!(settings.version, Settings::CURRENT_VERSION);
        assert_eq!(
            settings.connect_timeout_ms,
            Settings::MIN_CONNECT_TIMEOUT_MS
        );
        assert_eq!(settings.toggle_hotkey, None);
        assert_eq!(settings.preferred_device, None);
    }

    #[test]
    fn maps_to_controller_config() {
        let settings = Settings {
            auto_reconnect: false,
            connect_timeout_ms: 20_000,
            ..Settings::default()
        };
        let config = settings.controller_config();
        assert!(!config.auto_reconnect);
        assert_eq!(config.connect_timeout, Duration::from_secs(20));
        assert_eq!(config.reconnect, ReconnectPolicy::default());
    }
}
