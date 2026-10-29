//! Per-source volume on the buds' endpoint: the endpoint's own volume
//! (master), every other app's session (laptop) and BudsLink's own session
//! (the phone).
//!
//! Other apps' levels are only touched once the laptop slider leaves 100 %,
//! and each app's original level is remembered and given back.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use budslink_core::Level;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator, ISimpleAudioVolume,
};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::core::{Interface, PCWSTR, Result};

use crate::com::{take_pwstr, wide};

/// Remembers other apps' own levels so they can be given back.
#[derive(Default)]
pub(crate) struct SessionLevels {
    /// Session instance id -> (volume, muted) before BudsLink changed it.
    originals: Mutex<HashMap<String, (f32, bool)>>,
}

impl SessionLevels {
    /// Sets the buds' endpoint volume.
    pub(crate) fn set_master(
        enumerator: &IMMDeviceEnumerator,
        endpoint_id: &str,
        level: Level,
    ) -> Result<()> {
        // SAFETY: COM calls on live interfaces; `wide` outlives GetDevice.
        unsafe {
            let device = enumerator.GetDevice(PCWSTR(wide(endpoint_id).as_ptr()))?;
            let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
            volume.SetMasterVolumeLevelScalar(percent(level), std::ptr::null())?;
            volume.SetMute(level.muted, std::ptr::null())?;
        }
        Ok(())
    }

    /// Applies `laptop` to every other app's session on the endpoint and
    /// `own` to BudsLink's own session.
    pub(crate) fn apply(
        &self,
        enumerator: &IMMDeviceEnumerator,
        endpoint_id: &str,
        laptop: Level,
        own: Level,
    ) -> Result<()> {
        let own_pid = std::process::id();
        let mut originals = self
            .originals
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for (id, pid, volume) in sessions(enumerator, endpoint_id)? {
            // SAFETY: COM calls on a live session volume.
            unsafe {
                if pid == own_pid {
                    volume.SetMasterVolume(percent(own), std::ptr::null())?;
                    volume.SetMute(own.muted, std::ptr::null())?;
                    continue;
                }
                if laptop == Level::FULL && !originals.contains_key(&id) {
                    continue; // never touched, nothing to do
                }
                if !originals.contains_key(&id) {
                    let before = (volume.GetMasterVolume()?, volume.GetMute()?.as_bool());
                    originals.insert(id.clone(), before);
                }
                let (original, was_muted) = originals[&id];
                // Scale the app's own level, so the mixer balance stays.
                volume.SetMasterVolume(original * percent(laptop), std::ptr::null())?;
                volume.SetMute(laptop.muted || was_muted, std::ptr::null())?;
            }
        }
        Ok(())
    }

    /// Gives every app the level it had before BudsLink changed it.
    pub(crate) fn restore(
        &self,
        enumerator: &IMMDeviceEnumerator,
        endpoint_id: &str,
    ) -> Result<()> {
        let mut originals = self
            .originals
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        for (id, _, volume) in sessions(enumerator, endpoint_id)? {
            if let Some((level, muted)) = originals.remove(&id) {
                // SAFETY: COM calls on a live session volume.
                unsafe {
                    volume.SetMasterVolume(level, std::ptr::null())?;
                    volume.SetMute(muted, std::ptr::null())?;
                }
            }
        }
        originals.clear();
        Ok(())
    }
}

/// (instance id, process id, volume) for every session on the endpoint.
fn sessions(
    enumerator: &IMMDeviceEnumerator,
    endpoint_id: &str,
) -> Result<Vec<(String, u32, ISimpleAudioVolume)>> {
    let mut found = Vec::new();
    // SAFETY: COM calls on live interfaces; strings are freed by `take_pwstr`.
    unsafe {
        let device = enumerator.GetDevice(PCWSTR(wide(endpoint_id).as_ptr()))?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        let list = manager.GetSessionEnumerator()?;
        for index in 0..list.GetCount()? {
            let Ok(session) = list.GetSession(index) else {
                continue;
            };
            let Ok(control) = session.cast::<IAudioSessionControl2>() else {
                continue;
            };
            let Some(id) = control
                .GetSessionInstanceIdentifier()
                .ok()
                .and_then(|id| take_pwstr(id))
            else {
                continue;
            };
            let pid = control.GetProcessId().unwrap_or(0);
            if let Ok(volume) = session.cast::<ISimpleAudioVolume>() {
                found.push((id, pid, volume));
            }
        }
    }
    Ok(found)
}

/// The level as a 0.0 to 1.0 scalar, ignoring mute (set separately).
fn percent(level: Level) -> f32 {
    f32::from(level.percent.min(100)) / 100.0
}
