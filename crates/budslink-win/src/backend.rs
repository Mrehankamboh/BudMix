//! [`DeviceBackend`] implementation for Windows.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use budslink_core::{AudioDevice, BackendError, ChangeNotifier, DeviceBackend, DeviceId, Watch};
use windows::Win32::Foundation::{E_NOTIMPL, ERROR_NOT_FOUND, ERROR_SET_NOT_FOUND};
use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
use windows::Win32::Media::KernelStreaming::{
    KSPROPERTY_BTAUDIO, KSPROPERTY_ONESHOT_DISCONNECT, KSPROPERTY_ONESHOT_RECONNECT,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::HRESULT;

use crate::classify::group_endpoints;
use crate::notify::Registration;
use crate::{com, endpoints, ks};

/// Talks to paired Bluetooth audio devices through Core Audio and the
/// Bluetooth audio driver's `KSPROPSETID_BtAudio` property set.
#[derive(Default)]
pub struct WindowsBackend {
    /// KS filters per device, refreshed on every `list_devices`.
    filters: Mutex<HashMap<DeviceId, Vec<String>>>,
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn enumerator() -> Result<IMMDeviceEnumerator, BackendError> {
        com::ensure_initialized();
        // SAFETY: standard creation of the system MMDevice enumerator.
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(os_error)
    }

    fn filters_for(&self, id: &DeviceId) -> Result<Vec<String>, BackendError> {
        let cached = self.cache().get(id).cloned();
        let filters = match cached {
            Some(filters) => filters,
            None => {
                self.list_devices()?;
                self.cache().get(id).cloned().unwrap_or_default()
            }
        };
        if filters.is_empty() {
            Err(BackendError::NotFound(id.clone()))
        } else {
            Ok(filters)
        }
    }

    fn cache(&self) -> std::sync::MutexGuard<'_, HashMap<DeviceId, Vec<String>>> {
        self.filters.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sends the request to every filter of the device (A2DP and HFP); it
    /// succeeds if at least one driver accepts it.
    fn oneshot(&self, id: &DeviceId, property: KSPROPERTY_BTAUDIO) -> Result<(), BackendError> {
        let enumerator = Self::enumerator()?;
        let mut first_error = None;
        let mut accepted = false;
        for filter in self.filters_for(id)? {
            match ks::send_oneshot(&enumerator, &filter, property) {
                Ok(()) => accepted = true,
                Err(err) => {
                    log::debug!("KS request to {filter} failed: {err}");
                    first_error.get_or_insert(err);
                }
            }
        }
        match (accepted, first_error) {
            (true, _) | (false, None) => Ok(()),
            (false, Some(err)) if is_unsupported(err.code()) => Err(BackendError::NotSupported(
                "this Bluetooth driver does not accept connect/disconnect requests".into(),
            )),
            (false, Some(err)) => Err(os_error(err)),
        }
    }
}

impl DeviceBackend for WindowsBackend {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn list_devices(&self) -> Result<Vec<AudioDevice>, BackendError> {
        let enumerator = Self::enumerator()?;
        let endpoints = endpoints::bluetooth_endpoints(&enumerator).map_err(os_error)?;
        let groups = group_endpoints(endpoints);
        *self.cache() = groups
            .iter()
            .map(|group| (group.device.id.clone(), group.filter_ids.clone()))
            .collect();
        Ok(groups.into_iter().map(|group| group.device).collect())
    }

    fn connect(&self, id: &DeviceId) -> Result<(), BackendError> {
        self.oneshot(id, KSPROPERTY_ONESHOT_RECONNECT)
    }

    fn disconnect(&self, id: &DeviceId) -> Result<(), BackendError> {
        self.oneshot(id, KSPROPERTY_ONESHOT_DISCONNECT)
    }

    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError> {
        let registration = Registration::register(Self::enumerator()?, notify).map_err(os_error)?;
        Ok(Watch::new(registration))
    }
}

fn is_unsupported(code: HRESULT) -> bool {
    code == E_NOTIMPL
        || code == ERROR_SET_NOT_FOUND.to_hresult()
        || code == ERROR_NOT_FOUND.to_hresult()
}

fn os_error(err: windows::core::Error) -> BackendError {
    BackendError::Os {
        code: err.code().0,
        message: err.message().to_string(),
    }
}
