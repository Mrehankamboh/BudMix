//! Windows version checks.

use windows::Wdk::System::SystemServices::RtlGetVersion;
use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

/// First Windows 11 build; Mica is only available from here on.
const WINDOWS_11_BUILD: u32 = 22_000;

/// Real OS build number. Unlike `GetVersionExW`, `RtlGetVersion` is not
/// affected by compatibility manifests.
pub fn windows_build() -> Option<u32> {
    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a properly sized, writable OSVERSIONINFOW.
    let status = unsafe { RtlGetVersion(&mut info) };
    status.is_ok().then_some(info.dwBuildNumber)
}

/// Whether the window can use the Windows 11 Mica backdrop.
pub fn supports_mica() -> bool {
    windows_build().is_some_and(|build| build >= WINDOWS_11_BUILD)
}
