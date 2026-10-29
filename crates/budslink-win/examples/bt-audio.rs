//! Hardware check tool for the Windows backend (spikes S1 and S2 in
//! docs/FEASIBILITY.md).
//!
//! ```text
//! cargo run -p budslink-win --example bt-audio -- list
//! cargo run -p budslink-win --example bt-audio -- connect "{container-guid}"
//! cargo run -p budslink-win --example bt-audio -- disconnect "{container-guid}"
//! cargo run -p budslink-win --example bt-audio -- watch
//! ```

#[cfg(windows)]
fn main() {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use budslink_core::{DeviceBackend, DeviceId};
    use budslink_win::WindowsBackend;

    let backend = WindowsBackend::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let print = |backend: &WindowsBackend| match backend.list_devices() {
        Ok(devices) if devices.is_empty() => println!("No paired Bluetooth audio devices found."),
        Ok(devices) => {
            for device in devices {
                let state = if device.connected {
                    "connected"
                } else {
                    "disconnected"
                };
                println!(
                    "{:<14} {:<40} {:?}  {}",
                    state, device.id, device.profiles, device.name
                );
            }
        }
        Err(err) => eprintln!("listing failed: {err}"),
    };

    match args.first().map(String::as_str) {
        Some("list") | None => print(&backend),
        Some(verb @ ("connect" | "disconnect")) => {
            let Some(id) = args.get(1).map(DeviceId::new) else {
                eprintln!("usage: bt-audio {verb} <device id from `list`>");
                std::process::exit(2);
            };
            let want = verb == "connect";
            let started = Instant::now();
            let result = if want {
                backend.connect(&id)
            } else {
                backend.disconnect(&id)
            };
            if let Err(err) = result {
                eprintln!("{verb} request failed: {err}");
                std::process::exit(1);
            }
            println!("{verb} request accepted, waiting up to 20 s...");
            while started.elapsed() < Duration::from_secs(20) {
                let done = backend
                    .list_devices()
                    .unwrap_or_default()
                    .iter()
                    .any(|device| device.id == id && device.connected == want);
                if done {
                    println!("done in {:.1} s", started.elapsed().as_secs_f32());
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            eprintln!("state did not change within 20 s");
            std::process::exit(1);
        }
        Some("watch") => {
            let (tx, rx) = std::sync::mpsc::channel();
            let _watch = backend
                .watch(Arc::new(move || {
                    let _ = tx.send(());
                }))
                .expect("failed to register notifications");
            println!("Watching for changes, press Ctrl+C to stop.");
            print(&backend);
            while rx.recv().is_ok() {
                println!("--- change");
                print(&backend);
            }
        }
        Some(other) => {
            eprintln!("unknown command `{other}`; use list, connect, disconnect or watch");
            std::process::exit(2);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("bt-audio only works on Windows.");
}
