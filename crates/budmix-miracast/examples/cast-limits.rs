//! `cast-limits`: prints how many phones this laptop's Miracast receiver says
//! it can take at once. It only reads the status, so a running BudMix and any
//! cast keep going.
//!
//! ```text
//! cargo run -p budmix-miracast --example cast-limits
//! ```

#[cfg(not(windows))]
fn main() {
    eprintln!("cast-limits only runs on Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    let status = windows::Media::Miracast::MiracastReceiver::new()?.GetStatus()?;
    println!("wifi status:              {:?}", status.WiFiStatus()?);
    println!("listening status:         {:?}", status.ListeningStatus()?);
    println!(
        "max simultaneous phones:  {}",
        status.MaxSimultaneousConnections()?
    );
    println!(
        "takeover supported:       {}",
        status.IsConnectionTakeoverSupported()?
    );
    Ok(())
}
