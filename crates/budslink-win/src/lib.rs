//! Windows backend for BudsLink.
//!
//! Derived from BudsBridge (https://github.com/shakaibarsalan/BudsBridge, MIT,
//! Copyright (c) 2026 Muhammad Shakaib Arsalan).
//!
//! * Lists paired Bluetooth audio devices from Core Audio render endpoints,
//!   grouped by device container.
//! * Connects/disconnects them with `KSPROPSETID_BtAudio` one-shot requests.
//! * Watches endpoint state changes with `IMMNotificationClient`.
//! * Bridge mode: receives a paired phone's audio with
//!   `AudioPlaybackConnection` ([`PhoneAudioBackend`]), and lists paired
//!   phones Windows won't stream from.
//!
//! Only [`classify`] and [`paired`] are compiled on other platforms, so their
//! logic is tested everywhere.

pub mod classify;
pub mod paired;

#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod bluetooth;
#[cfg(windows)]
mod com;
#[cfg(windows)]
mod endpoints;
#[cfg(windows)]
mod ks;
#[cfg(windows)]
mod notify;
#[cfg(windows)]
mod os;
#[cfg(windows)]
mod phone;

#[cfg(windows)]
pub use backend::WindowsBackend;
#[cfg(windows)]
pub use os::{supports_mica, windows_build};
#[cfg(windows)]
pub use phone::PhoneAudioBackend;
