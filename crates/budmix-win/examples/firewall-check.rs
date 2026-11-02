//! `firewall-check`: prints what the setup check sees in Windows Firewall for
//! a BudMix.exe, and whether a casting phone's sound would get through. Only
//! reads, so it needs no admin rights.
//!
//! ```text
//! cargo run -p budmix-win --example firewall-check -- "C:\path\to\budmix.exe"
//! ```

#[cfg(not(windows))]
fn main() {
    eprintln!("firewall-check only runs on Windows.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    use std::process::Command;

    use budmix_win::checks::{
        cast_sound_allowed, encode_command, parse_powershell_report, report_script,
    };

    let program = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "budmix.exe".to_owned());
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &encode_command(&report_script(&program)),
        ])
        .output()
        .expect("can't start PowerShell");
    let json = String::from_utf8_lossy(&output.stdout);
    let report = parse_powershell_report(&json);
    println!("program:             {program}");
    println!("firewall on Public:  {:?}", report.firewall_public);
    for rule in &report.rules {
        println!("rule:                {rule:?}");
    }
    println!("cast sound allowed:  {:?}", cast_sound_allowed(&report));
}
