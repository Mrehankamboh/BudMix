//! `phone-mix`: opens the Bluetooth audio link of every paired phone at once
//! and shows which links stay up and how loud each audio session on the
//! default output is. It answers whether Windows plays several phones together.
//! Quit BudMix first, because it holds the same links.
//!
//! ```text
//! cargo run -p budmix-win --example phone-mix -- --seconds 60
//! ```

#[cfg(not(windows))]
fn main() {
    eprintln!("phone-mix only runs on Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use budmix_core::SourceBackend;
    use budmix_win::PhoneAudioBackend;
    use windows::Win32::Media::Audio::{IMMDeviceEnumerator, MMDeviceEnumerator};
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };

    let mut seconds = 60;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match (arg.as_str(), args.next().and_then(|n| n.parse().ok())) {
            ("--seconds", Some(limit)) => seconds = limit,
            _ => {
                eprintln!("usage: phone-mix [--seconds <run time>]");
                std::process::exit(2);
            }
        }
    }

    // SAFETY: plain COM initialisation for this thread, kept for its lifetime.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // SAFETY: creating the system's device enumerator.
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .expect("no audio device enumerator");

    let started = Instant::now();
    let stamp = || format!("[{:>5.1}s]", started.elapsed().as_secs_f32());
    let backend = PhoneAudioBackend::new();
    let phones = backend.list_sources().expect("can't list phones");
    println!("{} {} paired phone(s)", stamp(), phones.len());

    for phone in &phones {
        println!("{} opening {} ...", stamp(), phone.name);
        match backend.open(&phone.id) {
            Ok(()) => println!("{} {} OPEN", stamp(), phone.name),
            Err(err) => println!("{} {} FAILED: {err}", stamp(), phone.name),
        }
        report(&stamp(), &backend, &enumerator, &mut BTreeMap::new());
    }

    let mut last = BTreeMap::new();
    while started.elapsed() < Duration::from_secs(seconds) {
        std::thread::sleep(Duration::from_secs(2));
        report(&stamp(), &backend, &enumerator, &mut last);
    }
    for phone in &phones {
        let _ = backend.close(&phone.id);
    }
    println!("{} done", stamp());
}

/// Prints link changes, then the peak of every audio session.
#[cfg(windows)]
fn report(
    stamp: &str,
    backend: &budmix_win::PhoneAudioBackend,
    enumerator: &windows::Win32::Media::Audio::IMMDeviceEnumerator,
    last: &mut std::collections::BTreeMap<String, bool>,
) {
    use budmix_core::SourceBackend;

    for phone in backend.list_sources().unwrap_or_default() {
        if last.insert(phone.name.clone(), phone.streaming) != Some(phone.streaming) {
            let state = if phone.streaming { "open" } else { "CLOSED" };
            println!("{stamp} link: {} {state}", phone.name);
        }
    }
    match session_peaks(enumerator) {
        Ok(peaks) => println!("{stamp} sessions: {peaks}"),
        Err(err) => println!("{stamp} sessions: can't read ({err})"),
    }
}

/// `pid=peak` for every session on the default output; `*` marks this process.
#[cfg(windows)]
fn session_peaks(
    enumerator: &windows::Win32::Media::Audio::IMMDeviceEnumerator,
) -> windows::core::Result<String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
    use windows::Win32::Media::Audio::{
        IAudioSessionControl2, IAudioSessionManager2, eConsole, eRender,
    };
    use windows::Win32::System::Com::CLSCTX_ALL;
    use windows::core::Interface;

    let mut parts = Vec::new();
    // SAFETY: COM calls on live interfaces.
    unsafe {
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        let list = manager.GetSessionEnumerator()?;
        for index in 0..list.GetCount()? {
            let session = list.GetSession(index)?;
            let pid = session
                .cast::<IAudioSessionControl2>()?
                .GetProcessId()
                .unwrap_or(0);
            let peak = session.cast::<IAudioMeterInformation>()?.GetPeakValue()?;
            let own = if pid == std::process::id() { "*" } else { "" };
            parts.push(format!("{own}{pid}={peak:.2}"));
        }
    }
    Ok(parts.join(" "))
}
