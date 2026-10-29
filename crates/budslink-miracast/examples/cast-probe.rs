//! `cast-probe`: checks whether BudsLink's built-in Miracast receiver works on
//! this laptop, and plays a phone's cast audio on the default output.
//!
//! ```text
//! cargo run -p budslink-miracast --example cast-probe
//! cargo run -p budslink-miracast --example cast-probe -- --name "My Laptop" --pin
//! ```

#[cfg(not(windows))]
fn main() {
    eprintln!("cast-probe only runs on Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    use std::sync::OnceLock;
    use std::time::Instant;

    use budslink_miracast::{Receiver, ReceiverEvent, ReceiverOptions};

    static STARTED: OnceLock<Instant> = OnceLock::new();
    let stamp = || {
        format!(
            "[{:>6.1}s]",
            STARTED.get_or_init(Instant::now).elapsed().as_secs_f32()
        )
    };
    stamp();

    // The probe has no approval step: every phone plays at once.
    let mut options = ReceiverOptions {
        auto_play: true,
        ..ReceiverOptions::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--name" => match args.next() {
                Some(name) => options.friendly_name = name,
                None => usage(),
            },
            "--pin" => options.require_pin = true,
            _ => usage(),
        }
    }

    println!(
        "{} Starting the Miracast receiver \"{}\"...",
        stamp(),
        options.friendly_name
    );
    let receiver = match Receiver::start(&options, move |event| {
        let line = match event {
            ReceiverEvent::Status(listening) => format!("receiver status: {listening:?}"),
            ReceiverEvent::PhoneConnected {
                name,
                pin: Some(pin),
            } => {
                format!("phone connecting: {name}. Type this PIN on the phone: {pin}")
            }
            ReceiverEvent::PhoneConnected { name, pin: None } => {
                format!("phone connecting: {name}")
            }
            ReceiverEvent::StreamStarted { name } => {
                format!("stream from {name} is playing. Play something on the phone now.")
            }
            ReceiverEvent::PlaybackFailed { message } => format!("PLAYBACK FAILED: {message}"),
            ReceiverEvent::PhoneDisconnected { name } => {
                format!("phone disconnected: {name}. Ready for the next cast.")
            }
        };
        println!("{} {line}", stamp());
    }) {
        Ok(receiver) => receiver,
        Err(failure) => {
            println!("{} FAILED: {failure}", stamp());
            println!("  steps:   {}", failure.result);
            println!("  verdict: {:?}", failure.verdict());
            println!("  {}", failure.verdict().explain());
            std::process::exit(1);
        }
    };

    println!("{} steps:   {}", stamp(), receiver.result());
    println!("{} verdict: {:?}", stamp(), receiver.verdict());
    println!("  {}", receiver.verdict().explain());
    println!();
    println!("Keep this window open and cast from the phone. Press Enter to stop.");
    let mut line = String::new();
    let _ = std::io::stdin().read_line(&mut line);
    println!("{} last status: {:?}", stamp(), receiver.listening());
    drop(receiver);
    println!("{} Receiver stopped.", stamp());
}

#[cfg(windows)]
fn usage() -> ! {
    eprintln!("usage: cast-probe [--name <shown on the phone>] [--pin]");
    std::process::exit(2);
}
