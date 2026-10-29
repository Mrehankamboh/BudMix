//! Per-thread COM initialisation and small FFI helpers.

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{
    COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::core::PWSTR;

struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    fn enter() -> Self {
        // SAFETY: plain COM initialisation; balanced in `Drop` when it succeeds.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            // The thread already joined an STA (e.g. the UI thread). The
            // MMDevice API works from either apartment, so just use it.
            log::debug!("COM already initialised as STA on this thread");
        } else if let Err(err) = hr.ok() {
            log::warn!("CoInitializeEx failed: {err}");
        }
        Self {
            initialized: hr.is_ok(),
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: balances the successful `CoInitializeEx` above.
            unsafe { CoUninitialize() };
        }
    }
}

thread_local! {
    static APARTMENT: ComApartment = ComApartment::enter();
}

/// Makes sure COM is initialised on the calling thread.
pub(crate) fn ensure_initialized() {
    APARTMENT.with(|_| {});
}

/// Converts a COM-allocated string to `String` and frees it.
///
/// # Safety
/// `value` must be null or a valid string allocated with `CoTaskMemAlloc`.
pub(crate) unsafe fn take_pwstr(value: PWSTR) -> Option<String> {
    if value.is_null() {
        return None;
    }
    // SAFETY: guaranteed by the caller.
    let text = unsafe { value.to_string() }.ok();
    // SAFETY: the string was allocated by COM and is not used afterwards.
    unsafe { CoTaskMemFree(Some(value.0 as *const _)) };
    text
}

/// Null-terminated UTF-16 copy of `text`, for `PCWSTR` parameters.
pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
