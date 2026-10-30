//! Device model shared by every backend and the UI.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Stable identifier of a Bluetooth audio device.
///
/// On Windows this is the device container GUID, which groups the
/// "Headphones" (A2DP) and "Headset" (HFP) endpoints of one pair of buds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Bluetooth audio profile exposed by a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AudioProfile {
    /// A2DP: high-quality stereo playback, no microphone.
    Stereo,
    /// HFP/HSP: microphone plus low-quality mono audio.
    HandsFree,
    /// Bluetooth LE Audio (LC3).
    LeAudio,
    Unknown,
}

/// A paired Bluetooth audio device as reported by a backend.
///
/// `connected` is the backend's raw view; the [`crate::Controller`] layers
/// transitional states (connecting, disconnecting) on top of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: DeviceId,
    pub name: String,
    pub profiles: Vec<AudioProfile>,
    pub connected: bool,
}

impl AudioDevice {
    pub fn new(id: impl Into<String>, name: impl Into<String>, connected: bool) -> Self {
        Self {
            id: DeviceId::new(id),
            name: name.into(),
            profiles: vec![AudioProfile::Stereo],
            connected,
        }
    }

    pub fn with_profiles(mut self, profiles: impl IntoIterator<Item = AudioProfile>) -> Self {
        self.profiles = profiles.into_iter().collect();
        self.profiles.sort();
        self.profiles.dedup();
        self
    }
}

/// Connection state shown to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
}

impl ConnectionState {
    /// True while the device is connected or a connection is in progress.
    pub fn is_engaged(self) -> bool {
        matches!(self, Self::Connected | Self::Connecting)
    }

    pub fn is_pending(self) -> bool {
        matches!(self, Self::Connecting | Self::Disconnecting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_are_sorted_and_deduplicated() {
        let device = AudioDevice::new("id", "Buds", false).with_profiles([
            AudioProfile::HandsFree,
            AudioProfile::Stereo,
            AudioProfile::HandsFree,
        ]);
        assert_eq!(
            device.profiles,
            vec![AudioProfile::Stereo, AudioProfile::HandsFree]
        );
    }

    #[test]
    fn serializes_in_camel_case() {
        let device = AudioDevice::new("{abc}", "Buds", true);
        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "id": "{abc}",
                "name": "Buds",
                "profiles": ["stereo"],
                "connected": true
            })
        );
        assert_eq!(
            serde_json::to_value(ConnectionState::Disconnecting).unwrap(),
            "disconnecting"
        );
    }

    #[test]
    fn engaged_and_pending_states() {
        assert!(ConnectionState::Connected.is_engaged());
        assert!(ConnectionState::Connecting.is_engaged());
        assert!(!ConnectionState::Disconnecting.is_engaged());
        assert!(ConnectionState::Connecting.is_pending());
        assert!(ConnectionState::Disconnecting.is_pending());
        assert!(!ConnectionState::Connected.is_pending());
    }
}
