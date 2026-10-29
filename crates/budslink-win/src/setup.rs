//! Probes this PC for the setup checks and runs the fixes.
//!
//! Network, firewall and Wi-Fi details come from the same tools a user would
//! run (`Get-NetConnectionProfile`, `Get-NetFirewallProfile`, `netsh`), so
//! what BudsLink reports matches what the user can check by hand.

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use windows::Devices::Radios::{Radio, RadioKind, RadioState};

use crate::checks::{
    self, CheckResult, FixTask, PowerShellReport, WifiCapabilities, encode_command, fix_invocation,
};
use crate::com;

/// Keeps console windows from flashing up.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// What UAC returns when the user says No.
const ERROR_CANCELLED: i32 = 1223;

const REPORT_SCRIPT: &str = "\
$n = Get-NetConnectionProfile | Select-Object Name,InterfaceAlias,@{n='Category';e={\"$($_.NetworkCategory)\"}}
$f = (Get-NetFirewallProfile -Profile Private).Enabled
$r = @(Get-NetFirewallRule -DisplayName 'BudsLink (Private networks)' -ErrorAction SilentlyContinue).Count
[pscustomobject]@{ Networks = @($n); FirewallPrivate = \"$f\"; RuleCount = $r } | ConvertTo-Json -Depth 4 -Compress";

/// Where Windows keeps the Wireless Display receiver app once installed.
const WIRELESS_DISPLAY_DIR: &str = r"C:\Windows\SystemApps\Microsoft.PPIProjection_cw5n1h2txyewy";

/// Everything the system part of the setup check found.
#[derive(Debug, Clone, Default)]
pub struct SystemReport {
    pub bluetooth_on: Option<bool>,
    pub powershell: PowerShellReport,
    pub wifi: WifiCapabilities,
    pub wireless_display: Option<bool>,
}

impl SystemReport {
    /// Runs every probe. Takes a second or two (PowerShell and netsh).
    pub fn collect() -> Self {
        Self {
            bluetooth_on: bluetooth_radio_on(),
            powershell: powershell(REPORT_SCRIPT)
                .map(|json| checks::parse_powershell_report(&json))
                .unwrap_or_default(),
            wifi: run(Command::new("netsh").args(["wlan", "show", "wirelesscapabilities"]))
                .map(|text| checks::parse_wireless_capabilities(&text))
                .unwrap_or_default(),
            wireless_display: Path::new(WIRELESS_DISPLAY_DIR).exists().then_some(true),
        }
    }

    /// The system checks, in the order the wizard shows them.
    pub fn checks(&self) -> Vec<CheckResult> {
        vec![
            checks::bluetooth_check(self.bluetooth_on),
            checks::network_check(&self.powershell.networks),
            checks::firewall_check(self.powershell.firewall_private, self.powershell.rule_present),
            checks::miracast_check(self.wifi),
            checks::wireless_display_check(self.wireless_display),
        ]
    }

    /// Plain text for "Copy diagnostics".
    pub fn diagnostics(&self) -> String {
        let networks: Vec<String> = self
            .powershell
            .networks
            .iter()
            .map(|network| {
                format!(
                    "{} ({}, {})",
                    network.name, network.interface_alias, network.category
                )
            })
            .collect();
        [
            format!("Bluetooth radio on: {:?}", self.bluetooth_on),
            format!(
                "Networks: {}",
                if networks.is_empty() {
                    "none".to_owned()
                } else {
                    networks.join("; ")
                }
            ),
            format!(
                "Firewall (Private): {:?} · BudsLink rule: {}",
                self.powershell.firewall_private, self.powershell.rule_present
            ),
            format!(
                "Miracast sink: {:?} · Wi-Fi Direct GO: {:?}",
                self.wifi.miracast_sink, self.wifi.wifi_direct_go
            ),
            format!("Wireless Display app: {:?}", self.wireless_display),
        ]
        .join("\n")
    }
}

/// `Some(true)` if a Bluetooth radio is on, `Some(false)` if all are off,
/// `None` if there is none.
pub fn bluetooth_radio_on() -> Option<bool> {
    com::ensure_initialized();
    let radios = Radio::GetRadiosAsync().ok()?.join().ok()?;
    let mut found = None;
    for index in 0..radios.Size().ok()? {
        let Ok(radio) = radios.GetAt(index) else {
            continue;
        };
        if radio.Kind().ok() != Some(RadioKind::Bluetooth) {
            continue;
        }
        if radio.State().ok() == Some(RadioState::On) {
            return Some(true);
        }
        found = Some(false);
    }
    found
}

/// Opens a Windows Settings page such as `ms-settings:bluetooth`.
pub fn open_settings(uri: &str) -> Result<(), String> {
    if !uri.starts_with("ms-settings:") {
        return Err(format!("not a Settings page: {uri}"));
    }
    Command::new("explorer.exe")
        .arg(uri)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("Couldn't open Settings: {err}"))
}

/// Runs the admin fixes with one UAC prompt and waits for them.
pub fn run_fixes(tasks: &[FixTask], network: &str, program: &Path) -> Result<(), String> {
    let encoded = encode_command(&fix_invocation(tasks, network, &program.to_string_lossy()));
    let elevate = format!(
        "try {{ $p = Start-Process -FilePath powershell.exe -Verb RunAs -Wait -PassThru \
         -WindowStyle Hidden -ArgumentList '-NoProfile -ExecutionPolicy Bypass -EncodedCommand {encoded}'; \
         exit $p.ExitCode }} catch {{ exit {ERROR_CANCELLED} }}"
    );
    let status = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &elevate])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|err| format!("Couldn't start PowerShell: {err}"))?;
    match status.code() {
        Some(0) => Ok(()),
        Some(ERROR_CANCELLED) => Err("The admin prompt was cancelled, so nothing changed.".into()),
        Some(failed) => Err(format!(
            "{failed} fix(es) failed. Details are in %TEMP%\\budslink-fix.log."
        )),
        None => Err("The fix was interrupted.".into()),
    }
}

fn powershell(script: &str) -> Option<String> {
    run(Command::new("powershell.exe").args([
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        &encode_command(script),
    ]))
}

fn run(command: &mut Command) -> Option<String> {
    let output = command
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .inspect_err(|err| log::warn!("running {command:?} failed: {err}"))
        .ok()?;
    if !output.status.success() {
        log::warn!("{command:?} exited with {}", output.status);
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}
