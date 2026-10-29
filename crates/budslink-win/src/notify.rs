//! Endpoint change notifications (connect, disconnect, pair, unpair, and a
//! new default playback device).

use budslink_core::ChangeNotifier;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, eRender,
};
use windows::core::{PCWSTR, Result, implement};

/// Forwards relevant Core Audio events to BudsLink.
///
/// Callbacks run on Core Audio's threads and must not block or call back
/// into the MMDevice API, so they only fire the notifier.
#[implement(IMMNotificationClient)]
struct EndpointListener {
    notify: ChangeNotifier,
}

impl IMMNotificationClient_Impl for EndpointListener_Impl {
    fn OnDeviceStateChanged(&self, _id: &PCWSTR, _state: DEVICE_STATE) -> Result<()> {
        (self.notify)();
        Ok(())
    }

    fn OnDeviceAdded(&self, _id: &PCWSTR) -> Result<()> {
        (self.notify)();
        Ok(())
    }

    fn OnDeviceRemoved(&self, _id: &PCWSTR) -> Result<()> {
        (self.notify)();
        Ok(())
    }

    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, _role: ERole, _id: &PCWSTR) -> Result<()> {
        // The laptop-audio router follows the default playback device.
        if flow == eRender {
            (self.notify)();
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _id: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

/// Keeps the listener registered until dropped.
pub(crate) struct Registration {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
}

// SAFETY: MMDevice objects are free-threaded; the registration is only
// moved between threads, never used concurrently.
unsafe impl Send for Registration {}

impl Registration {
    pub(crate) fn register(
        enumerator: IMMDeviceEnumerator,
        notify: ChangeNotifier,
    ) -> Result<Self> {
        let client: IMMNotificationClient = EndpointListener { notify }.into();
        // SAFETY: the client stays alive for as long as it is registered.
        unsafe { enumerator.RegisterEndpointNotificationCallback(&client)? };
        Ok(Self { enumerator, client })
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // SAFETY: unregisters the client registered in `register`.
        if let Err(err) = unsafe {
            self.enumerator
                .UnregisterEndpointNotificationCallback(&self.client)
        } {
            log::warn!("failed to unregister endpoint notifications: {err}");
        }
    }
}
