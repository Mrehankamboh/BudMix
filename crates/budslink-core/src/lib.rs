//! Platform-independent core of BudsLink.
//!
//! Derived from BudsBridge (https://github.com/shakaibarsalan/BudsBridge, MIT,
//! Copyright (c) 2026 Muhammad Shakaib Arsalan).
//!
//! * [`device`]: what a paired Bluetooth audio device looks like.
//! * [`controller`]: the pure connection state machine and reconnect logic.
//! * [`runtime`]: runs the controller against a [`backend::DeviceBackend`].
//! * [`bridge`], [`bridge_runtime`]: bridge mode, phone audio played through
//!   the laptop so the buds hear both without switching.
//! * [`settings`]: persisted user preferences.

pub mod backend;
pub mod bridge;
pub mod bridge_runtime;
pub mod controller;
pub mod device;
#[cfg(feature = "fake")]
pub mod fake;
pub mod reconnect;
pub mod runtime;
pub mod settings;

pub use backend::{BackendError, ChangeNotifier, DeviceBackend, Watch};
pub use bridge::{
    AudioSource, BridgeState, SourceBackend, SourceView, UnavailableReason, UnavailableSource,
};
pub use bridge_runtime::{BridgeHandle, BridgeRuntime, SourceListener};
pub use controller::{Action, Command, Controller, ControllerConfig, ControllerError, DeviceView};
pub use device::{AudioDevice, AudioProfile, ConnectionState, DeviceId};
pub use reconnect::ReconnectPolicy;
pub use runtime::{Runtime, RuntimeHandle, UpdateListener};
pub use settings::{Settings, SettingsError};
