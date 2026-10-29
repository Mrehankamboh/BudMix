//! [`AudioBackend`] for Windows: the default playback device, the buds'
//! playback endpoint, the call-mode (hands-free) check and the test tone.

use budslink_core::{
    AudioBackend, AudioProfile, BackendError, ChangeNotifier, DeviceId, Endpoint, Level, Watch,
};
use windows::Win32::Foundation::ERROR_NOT_FOUND;
use windows::Win32::Media::Audio::{
    AudioSessionStateActive, DEVICE_STATE_ACTIVE, IAudioSessionManager2, IMMDeviceEnumerator,
    MMDeviceEnumerator, eCapture, eMultimedia, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::PCWSTR;

use crate::classify::EndpointInfo;
use crate::com::{self, wide};
use crate::notify::Registration;
use crate::volume::SessionLevels;
use crate::{endpoints, policy, tone};

/// Core Audio based [`AudioBackend`].
#[derive(Default)]
pub struct WindowsAudioBackend {
    levels: SessionLevels,
}

impl WindowsAudioBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn enumerator() -> Result<IMMDeviceEnumerator, BackendError> {
        com::ensure_initialized();
        // SAFETY: standard creation of the system MMDevice enumerator.
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(os_error)
    }

    /// Active Bluetooth endpoints of one direction that belong to `buds`.
    fn buds_endpoints(
        enumerator: &IMMDeviceEnumerator,
        buds: &DeviceId,
        capture: bool,
    ) -> Result<Vec<(String, EndpointInfo)>, BackendError> {
        let flow = if capture { eCapture } else { eRender };
        let endpoints =
            endpoints::bluetooth_endpoints_with_ids(enumerator, flow, DEVICE_STATE_ACTIVE)
                .map_err(os_error)?;
        Ok(endpoints
            .into_iter()
            .filter(|(_, info)| info.container_id.eq_ignore_ascii_case(buds.as_str()))
            .collect())
    }
}

impl AudioBackend for WindowsAudioBackend {
    fn default_output(&self) -> Result<Option<Endpoint>, BackendError> {
        let enumerator = Self::enumerator()?;
        // SAFETY: COM call on a live enumerator.
        let device = match unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) } {
            Ok(device) => device,
            // No playback device at all.
            Err(err) if err.code() == ERROR_NOT_FOUND.to_hresult() => return Ok(None),
            Err(err) => return Err(os_error(err)),
        };
        let Some(id) = endpoints::endpoint_id(&device) else {
            return Ok(None);
        };
        let name = endpoints::endpoint_name(&device).unwrap_or_else(|| id.clone());
        Ok(Some(Endpoint::new(id, name)))
    }

    fn buds_output(&self, buds: &DeviceId) -> Result<Option<Endpoint>, BackendError> {
        let enumerator = Self::enumerator()?;
        let mut endpoints = Self::buds_endpoints(&enumerator, buds, false)?;
        // Stereo (A2DP) or LE Audio first; never pick a Windows 10 style
        // separate hands-free endpoint when a stereo one exists.
        endpoints.sort_by_key(|(_, info)| match info.profile {
            AudioProfile::Stereo | AudioProfile::LeAudio => 0,
            AudioProfile::Unknown => 1,
            AudioProfile::HandsFree => 2,
        });
        Ok(endpoints.into_iter().next().map(|(id, info)| {
            // SAFETY: COM call on a live enumerator; `wide` outlives it.
            let name = unsafe { enumerator.GetDevice(PCWSTR(wide(&id).as_ptr())) }
                .ok()
                .and_then(|device| endpoints::endpoint_name(&device))
                .unwrap_or(info.name);
            Endpoint::new(id, name)
        }))
    }

    fn set_default_output(&self, endpoint_id: &str) -> Result<(), BackendError> {
        policy::set_default_output(endpoint_id).map_err(os_error)
    }

    fn call_mode(&self, buds: &DeviceId) -> Result<bool, BackendError> {
        let enumerator = Self::enumerator()?;
        for (id, _) in Self::buds_endpoints(&enumerator, buds, true)? {
            // SAFETY: COM calls on live interfaces; `wide` outlives GetDevice.
            let active = unsafe {
                let device = enumerator
                    .GetDevice(PCWSTR(wide(&id).as_ptr()))
                    .map_err(os_error)?;
                let manager: IAudioSessionManager2 =
                    device.Activate(CLSCTX_ALL, None).map_err(os_error)?;
                let sessions = manager.GetSessionEnumerator().map_err(os_error)?;
                let count = sessions.GetCount().map_err(os_error)?;
                (0..count).any(|index| {
                    sessions
                        .GetSession(index)
                        .and_then(|session| session.GetState())
                        .is_ok_and(|state| state == AudioSessionStateActive)
                })
            };
            if active {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn set_master_volume(&self, endpoint_id: &str, level: Level) -> Result<(), BackendError> {
        SessionLevels::set_master(&Self::enumerator()?, endpoint_id, level).map_err(os_error)
    }

    fn apply_session_volumes(
        &self,
        endpoint_id: &str,
        laptop: Level,
        own: Level,
    ) -> Result<(), BackendError> {
        self.levels
            .apply(&Self::enumerator()?, endpoint_id, laptop, own)
            .map_err(os_error)
    }

    fn restore_session_volumes(&self, endpoint_id: &str) -> Result<(), BackendError> {
        self.levels
            .restore(&Self::enumerator()?, endpoint_id)
            .map_err(os_error)
    }

    fn play_tone(&self, endpoint_id: Option<&str>) -> Result<(), BackendError> {
        tone::play(endpoint_id).map_err(os_error)
    }

    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError> {
        let registration = Registration::register(Self::enumerator()?, notify).map_err(os_error)?;
        Ok(Watch::new(registration))
    }
}

fn os_error(err: windows::core::Error) -> BackendError {
    BackendError::Os {
        code: err.code().0,
        message: err.message().to_string(),
    }
}
