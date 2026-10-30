//! Platform-independent core of BudMix.
//!
//! Derived from BudsBridge (https://github.com/shakaibarsalan/BudsBridge, MIT,
//! Copyright (c) 2026 Muhammad Shakaib Arsalan).
//!
//! * [`device`]: what a paired Bluetooth audio device looks like.
//! * [`controller`]: the pure connection state machine and reconnect logic.
//! * [`runtime`]: runs the controller against a [`backend::DeviceBackend`].
//! * [`bridge`], [`bridge_runtime`]: bridge mode, phone audio played through
//!   the laptop so the buds hear both without switching.
//! * [`routing`], [`audio_runtime`]: laptop audio in the buds (buds as the
//!   default output, restored afterwards) and the call-mode (HFP) check.
//! * [`tone`]: the test beep.
//! * [`cast`], [`phone_router`]: phone audio over Wi-Fi (Miracast receiver,
//!   approvals) and the Auto Wi-Fi/Bluetooth rule that keeps one phone from
//!   playing twice.
//! * [`volume`]: per-source volume.
//! * [`settings`]: persisted user preferences.

pub mod audio_runtime;
pub mod backend;
pub mod bridge;
pub mod bridge_runtime;
pub mod cast;
pub mod cast_runtime;
pub mod controller;
pub mod device;
#[cfg(feature = "fake")]
pub mod fake;
pub mod phone_router;
pub mod reconnect;
pub mod routing;
pub mod runtime;
pub mod settings;
pub mod tone;
pub mod volume;

pub use audio_runtime::{AudioHandle, AudioListener, AudioRuntime, AudioView};
pub use backend::{BackendError, ChangeNotifier, DeviceBackend, Watch};
pub use bridge::{
    AudioSource, BridgeState, SourceBackend, SourceView, UnavailableReason, UnavailableSource,
};
pub use bridge_runtime::{BridgeHandle, BridgeRuntime, SourceListener};
pub use cast::{CastAction, CastController, CastPhase, CastView, ReceiverState, StartOutcome};
pub use cast_runtime::{
    CastBackend, CastConfig, CastEvent, CastEvents, CastHandle, CastListener, CastRuntime,
    CastStatus,
};
pub use controller::{Action, Command, Controller, ControllerConfig, ControllerError, DeviceView};
pub use device::{AudioDevice, AudioProfile, ConnectionState, DeviceId};
pub use phone_router::{PhoneMode, PhoneRouter};
pub use reconnect::ReconnectPolicy;
pub use routing::{AudioBackend, Endpoint, OutputRouter, RouteAction, route_target};
pub use runtime::{Runtime, RuntimeHandle, UpdateListener};
pub use settings::{Settings, SettingsError};
pub use volume::{Level, VolumeSource, Volumes};
