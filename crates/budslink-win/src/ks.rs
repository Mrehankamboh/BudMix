//! Bluetooth connect/disconnect requests through kernel streaming.
//!
//! Windows has no public "connect this headset" API. Bluetooth audio
//! drivers instead implement `KSPROPSETID_BtAudio` with
//! `KSPROPERTY_ONESHOT_RECONNECT` / `KSPROPERTY_ONESHOT_DISCONNECT`, the
//! same mechanism behind *Sound settings -> Connect*. The technique follows
//! the open-source ToothTray project (BSD-2-Clause, Jing Xia).

use std::ptr;

use windows::Win32::Media::Audio::IMMDeviceEnumerator;
use windows::Win32::Media::KernelStreaming::{
    IKsControl, KSIDENTIFIER, KSIDENTIFIER_0_0, KSPROPERTY_BTAUDIO, KSPROPERTY_TYPE_GET,
    KSPROPSETID_BtAudio,
};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::core::{PCWSTR, Result};

use crate::com::wide;

/// Sends a one-shot `KSPROPSETID_BtAudio` request to the given KS filter.
///
/// The call returns as soon as the driver accepts the request; the actual
/// connection change is reported later through endpoint state changes.
pub(crate) fn send_oneshot(
    enumerator: &IMMDeviceEnumerator,
    filter_id: &str,
    property: KSPROPERTY_BTAUDIO,
) -> Result<()> {
    let id = wide(filter_id);
    let mut request = KSIDENTIFIER::default();
    request.Anonymous.Anonymous = KSIDENTIFIER_0_0 {
        Set: KSPROPSETID_BtAudio,
        Id: property.0 as u32,
        Flags: KSPROPERTY_TYPE_GET,
    };
    let mut returned = 0u32;
    // SAFETY: `id` outlives the call; the request is a valid KSPROPERTY and
    // the property carries no data buffer.
    unsafe {
        let filter = enumerator.GetDevice(PCWSTR(id.as_ptr()))?;
        let control: IKsControl = filter.Activate(CLSCTX_ALL, None)?;
        control.KsProperty(
            &request,
            size_of::<KSIDENTIFIER>() as u32,
            ptr::null_mut(),
            0,
            &mut returned,
        )
    }
}
