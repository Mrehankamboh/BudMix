//! Hardware check tool for the Windows backend (spikes S1 and S2 in
//! docs/FEASIBILITY.md).
//!
//! ```text
//! cargo run -p budmix-win --example bt-audio -- list
//! cargo run -p budmix-win --example bt-audio -- connect "{container-guid}"
//! cargo run -p budmix-win --example bt-audio -- disconnect "{container-guid}"
//! cargo run -p budmix-win --example bt-audio -- watch
//! cargo run -p budmix-win --example bt-audio -- status "{container-guid}"
//! cargo run -p budmix-win --example bt-audio -- tone ["{container-guid}"]
//! cargo run -p budmix-win --example bt-audio -- make-default "{container-guid}"
//! ```

#[cfg(windows)]
fn main() {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use budmix_core::{AudioBackend, DeviceBackend, DeviceId};
    use budmix_win::{WindowsAudioBackend, WindowsBackend};

    let audio = WindowsAudioBackend::new();

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
        Some("status") => {
            match audio.default_output() {
                Ok(Some(output)) => println!("default output: {}  ({})", output.name, output.id),
                Ok(None) => println!("default output: none"),
                Err(err) => eprintln!("reading the default output failed: {err}"),
            }
            if let Some(id) = args.get(1).map(DeviceId::new) {
                match audio.buds_output(&id) {
                    Ok(Some(output)) => {
                        println!("buds output:    {}  ({})", output.name, output.id)
                    }
                    Ok(None) => println!("buds output:    none (not connected?)"),
                    Err(err) => eprintln!("finding the buds output failed: {err}"),
                }
                match audio.call_mode(&id) {
                    Ok(true) => {
                        println!("call mode:      YES, an app is using the buds' mic (HFP)")
                    }
                    Ok(false) => println!("call mode:      no (stereo quality)"),
                    Err(err) => eprintln!("call-mode check failed: {err}"),
                }
            }
        }
        Some("tone") => {
            let target = match args.get(1).map(DeviceId::new) {
                Some(id) => match audio.buds_output(&id) {
                    Ok(Some(output)) => Some(output.id),
                    Ok(None) => {
                        eprintln!("those buds are not connected");
                        std::process::exit(1);
                    }
                    Err(err) => {
                        eprintln!("finding the buds output failed: {err}");
                        std::process::exit(1);
                    }
                },
                None => None,
            };
            println!("Playing a 440 Hz beep...");
            if let Err(err) = audio.play_tone(target.as_deref()) {
                eprintln!("playing the beep failed: {err}");
                std::process::exit(1);
            }
            println!("Done. Did you hear it?");
        }
        Some("make-default") => {
            let Some(id) = args.get(1).map(DeviceId::new) else {
                eprintln!("usage: bt-audio make-default <device id from `list`>");
                std::process::exit(2);
            };
            let output = match audio.buds_output(&id) {
                Ok(Some(output)) => output,
                Ok(None) => {
                    eprintln!("those buds are not connected");
                    std::process::exit(1);
                }
                Err(err) => {
                    eprintln!("finding the buds output failed: {err}");
                    std::process::exit(1);
                }
            };
            match audio.set_default_output(&output.id) {
                Ok(()) => println!("{} is now the default output.", output.name),
                Err(err) => {
                    eprintln!("changing the default output failed: {err}");
                    std::process::exit(1);
                }
            }
        }
        Some(other) => {
            eprintln!(
                "unknown command `{other}`; use list, connect, disconnect, watch, status, tone or make-default"
            );
            std::process::exit(2);
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("bt-audio only works on Windows.");
}
