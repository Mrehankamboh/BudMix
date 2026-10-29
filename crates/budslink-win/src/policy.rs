//! Sets the default playback device.
//!
//! Windows has no documented API for this. `IPolicyConfig` is the COM
//! interface the Sound control panel itself uses; tools such as EarTrumpet
//! and AudioDeviceCmdlets rely on it too. It is undocumented, so a future
//! Windows update could change it: failures are reported, never hidden.

// The vtable declares methods BudsLink never calls, with their Windows names.
#![allow(dead_code, non_snake_case)]

use std::ffi::c_void;

use windows::Win32::Media::Audio::{ERole, eConsole, eMultimedia};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl, PCWSTR, Result, interface};

use crate::com::{self, wide};

/// `CPolicyConfigClient`.
const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

/// Vtable layout of `IPolicyConfig` (Windows 7 and later). Only
/// `SetDefaultEndpoint` is called; the methods before it must still be
/// declared, in order, for the layout to be right.
#[interface("f8679f50-850a-41cf-9c72-430f290290c8")]
unsafe trait IPolicyConfig: IUnknown {
    unsafe fn GetMixFormat(&self, device: PCWSTR, format: *mut *mut c_void) -> HRESULT;
    unsafe fn GetDeviceFormat(
        &self,
        device: PCWSTR,
        default: i32,
        format: *mut *mut c_void,
    ) -> HRESULT;
    unsafe fn ResetDeviceFormat(&self, device: PCWSTR) -> HRESULT;
    unsafe fn SetDeviceFormat(
        &self,
        device: PCWSTR,
        endpoint_format: *mut c_void,
        mix_format: *mut c_void,
    ) -> HRESULT;
    unsafe fn GetProcessingPeriod(
        &self,
        device: PCWSTR,
        default: i32,
        default_period: *mut i64,
        minimum_period: *mut i64,
    ) -> HRESULT;
    unsafe fn SetProcessingPeriod(&self, device: PCWSTR, period: *mut i64) -> HRESULT;
    unsafe fn GetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
    unsafe fn SetShareMode(&self, device: PCWSTR, mode: *mut c_void) -> HRESULT;
    unsafe fn GetPropertyValue(
        &self,
        device: PCWSTR,
        key: *const c_void,
        value: *mut c_void,
    ) -> HRESULT;
    unsafe fn SetPropertyValue(
        &self,
        device: PCWSTR,
        key: *const c_void,
        value: *mut c_void,
    ) -> HRESULT;
    unsafe fn SetDefaultEndpoint(&self, device: PCWSTR, role: ERole) -> HRESULT;
    unsafe fn SetEndpointVisibility(&self, device: PCWSTR, visible: i32) -> HRESULT;
}

/// Makes `endpoint_id` the default output for media and general sound
/// (`eConsole` and `eMultimedia`).
///
/// The communications role is left alone on purpose: an app that uses the
/// communications device usually opens its microphone too, and that would
/// switch classic Bluetooth buds to the low-quality hands-free profile.
pub(crate) fn set_default_output(endpoint_id: &str) -> Result<()> {
    com::ensure_initialized();
    let id = wide(endpoint_id);
    // SAFETY: plain COM activation and calls; `id` outlives the calls and
    // is null-terminated.
    unsafe {
        let policy: IPolicyConfig =
            CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)?;
        for role in [eConsole, eMultimedia] {
            policy.SetDefaultEndpoint(PCWSTR(id.as_ptr()), role).ok()?;
        }
    }
    Ok(())
}
