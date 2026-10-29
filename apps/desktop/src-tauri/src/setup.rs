//! The setup wizard: checks what BudsLink needs from this PC and runs the
//! fixes the user clicks.

use std::thread;
use std::time::Duration;

use budslink_win::checks::{self, CheckResult, Fix, NetworkProfile, WifiCapabilities};
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Every check, system ones first, then pairing.
pub fn run_checks(app: &AppHandle) -> Vec<CheckResult> {
    let state = app.state::<AppState>();
    let buds: Vec<String> = state
        .runtime()
        .views()
        .into_iter()
        .map(|device| device.name)
        .collect();
    let phones: Vec<String> = state
        .bridge()
        .views()
        .into_iter()
        .map(|phone| phone.name)
        .collect();
    let unavailable: Vec<String> = state
        .sources()
        .unavailable()
        .into_iter()
        .map(|phone| phone.name)
        .collect();

    let mut results = if is_demo(&state) {
        demo_system_checks()
    } else {
        system_checks()
    };
    results.insert(1, checks::buds_paired_check(&buds));
    results.insert(2, checks::phone_paired_check(&phones, &unavailable));
    results
}

/// Runs one fix. Admin fixes show one UAC prompt and wait for it.
pub fn run_fix(app: &AppHandle, fix: Fix) -> Result<(), String> {
    let state = app.state::<AppState>();
    if is_demo(&state) {
        log::info!("demo mode: pretending to run {fix:?}");
        thread::sleep(Duration::from_millis(800));
        return Ok(());
    }
    log::info!("running setup fix {fix:?}");
    apply(fix)
}

/// System part of "Copy diagnostics".
pub fn diagnostics(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    if is_demo(&state) {
        return "System checks: demo mode".to_owned();
    }
    system_diagnostics()
}

fn is_demo(state: &AppState) -> bool {
    state.runtime().backend_name() == "demo"
}

/// Sample results so the wizard can be tried without hardware.
fn demo_system_checks() -> Vec<CheckResult> {
    let home = NetworkProfile {
        name: "Demo Home Wi-Fi".into(),
        interface_alias: "Wi-Fi".into(),
        category: "Public".into(),
    };
    vec![
        checks::bluetooth_check(Some(true)),
        checks::network_check(&[home]),
        checks::firewall_check(Some(true), false),
        checks::miracast_check(WifiCapabilities {
            miracast_sink: Some(true),
            wifi_direct_go: Some(true),
        }),
        checks::wireless_display_check(None),
    ]
}

#[cfg(windows)]
fn system_checks() -> Vec<CheckResult> {
    budslink_win::setup::SystemReport::collect().checks()
}

#[cfg(windows)]
fn system_diagnostics() -> String {
    budslink_win::setup::SystemReport::collect().diagnostics()
}

#[cfg(windows)]
fn apply(fix: Fix) -> Result<(), String> {
    use budslink_win::checks::FixTask;
    use budslink_win::setup;

    let program =
        std::env::current_exe().map_err(|err| format!("Couldn't find BudsLink.exe: {err}"))?;
    match fix {
        Fix::OpenSettings { uri } => setup::open_settings(&uri),
        Fix::MakeNetworkPrivate { network } => {
            setup::run_fixes(&[FixTask::Network], &network, &program)
        }
        Fix::AddFirewallRule => setup::run_fixes(&[FixTask::Firewall], "", &program),
        Fix::InstallWirelessDisplay => setup::run_fixes(&[FixTask::WirelessDisplay], "", &program),
    }
}

#[cfg(not(windows))]
fn system_checks() -> Vec<CheckResult> {
    demo_system_checks()
}

#[cfg(not(windows))]
fn system_diagnostics() -> String {
    "System checks: not Windows".to_owned()
}

#[cfg(not(windows))]
fn apply(fix: Fix) -> Result<(), String> {
    Err(format!("{fix:?} is only available on Windows"))
}
