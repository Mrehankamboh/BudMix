//! The setup checks: what BudsLink needs from this PC, how to tell from what
//! Windows tools print, and how each problem can be fixed.
//!
//! Pure logic, compiled on every OS so it is unit-tested everywhere. The
//! Windows probing that feeds it lives in `setup.rs`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckId {
    Bluetooth,
    BudsPaired,
    PhonePaired,
    NetworkPrivate,
    Firewall,
    Miracast,
    WirelessDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Ok,
    /// Works, but something is missing or not ideal.
    Warning,
    /// BudsLink can't do this part until it is fixed.
    Problem,
    /// Only needed in some cases.
    Optional,
    /// Windows did not say.
    Unknown,
}

/// What the "Fix" button next to a check does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fix {
    /// Opens a page of Windows Settings, e.g. `ms-settings:bluetooth`.
    OpenSettings { uri: String },
    /// Marks this Wi-Fi network Private (needs admin, one prompt).
    MakeNetworkPrivate { network: String },
    /// Allows BudsLink through Windows Firewall on Private networks only
    /// (needs admin, one prompt).
    AddFirewallRule,
    /// Installs Windows' Wireless Display receiver (needs admin and
    /// internet, one time).
    InstallWirelessDisplay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub id: CheckId,
    pub status: CheckStatus,
    pub title: String,
    pub detail: String,
    pub fix: Option<Fix>,
}

impl CheckResult {
    fn new(id: CheckId, status: CheckStatus, title: &str, detail: impl Into<String>) -> Self {
        Self {
            id,
            status,
            title: title.to_owned(),
            detail: detail.into(),
            fix: None,
        }
    }

    fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }
}

const BLUETOOTH_SETTINGS: &str = "ms-settings:bluetooth";

pub fn bluetooth_check(radio_on: Option<bool>) -> CheckResult {
    let open = Fix::OpenSettings {
        uri: BLUETOOTH_SETTINGS.into(),
    };
    match radio_on {
        Some(true) => CheckResult::new(CheckId::Bluetooth, CheckStatus::Ok, "Bluetooth is on", ""),
        Some(false) => CheckResult::new(
            CheckId::Bluetooth,
            CheckStatus::Problem,
            "Bluetooth is off",
            "Turn Bluetooth on so BudsLink can reach your buds and phone.",
        )
        .with_fix(open),
        None => CheckResult::new(
            CheckId::Bluetooth,
            CheckStatus::Problem,
            "No Bluetooth radio found",
            "Windows reports no Bluetooth adapter. Check that it is enabled in Device Manager.",
        )
        .with_fix(open),
    }
}

pub fn buds_paired_check(buds: &[String]) -> CheckResult {
    if buds.is_empty() {
        return CheckResult::new(
            CheckId::BudsPaired,
            CheckStatus::Problem,
            "Buds not paired",
            "Pair your buds once: Windows Settings > Bluetooth & devices > Add device.",
        )
        .with_fix(Fix::OpenSettings {
            uri: BLUETOOTH_SETTINGS.into(),
        });
    }
    CheckResult::new(
        CheckId::BudsPaired,
        CheckStatus::Ok,
        "Buds paired",
        buds.join(", "),
    )
}

/// `streamable`: phones Windows can play audio from; `unavailable`: paired
/// phones it can't.
pub fn phone_paired_check(streamable: &[String], unavailable: &[String]) -> CheckResult {
    if !streamable.is_empty() {
        return CheckResult::new(
            CheckId::PhonePaired,
            CheckStatus::Ok,
            "Phone paired",
            format!("{} (Bluetooth fallback ready)", streamable.join(", ")),
        );
    }
    let fix = Fix::OpenSettings {
        uri: BLUETOOTH_SETTINGS.into(),
    };
    if !unavailable.is_empty() {
        return CheckResult::new(
            CheckId::PhonePaired,
            CheckStatus::Warning,
            "Phone paired, but Windows won't take its audio",
            format!(
                "{}: remove it in Bluetooth settings and pair it again from the laptop.",
                unavailable.join(", ")
            ),
        )
        .with_fix(fix);
    }
    CheckResult::new(
        CheckId::PhonePaired,
        CheckStatus::Warning,
        "Phone not paired",
        "Optional for Wi-Fi casting, needed for the automatic Bluetooth fallback.",
    )
    .with_fix(fix)
}

/// One network connection, as `Get-NetConnectionProfile` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct NetworkProfile {
    pub name: String,
    pub interface_alias: String,
    /// "Public", "Private" or "DomainAuthenticated".
    pub category: String,
}

/// Parses the JSON from `Get-NetConnectionProfile | Select-Object Name,
/// InterfaceAlias, @{n='Category';e={"$($_.NetworkCategory)"}} |
/// ConvertTo-Json`, which is a single object for one connection and an array
/// for several.
pub fn parse_network_profiles(json: &str) -> Vec<NetworkProfile> {
    let json = json.trim();
    if json.is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<Vec<NetworkProfile>>(json)
        .or_else(|_| serde_json::from_str::<NetworkProfile>(json).map(|profile| vec![profile]))
        .unwrap_or_default()
}

fn is_wifi(profile: &NetworkProfile) -> bool {
    let alias = profile.interface_alias.to_lowercase();
    alias.contains("wi-fi")
        || alias.contains("wifi")
        || alias.contains("wlan")
        || alias.contains("wireless")
}

pub fn network_check(profiles: &[NetworkProfile]) -> CheckResult {
    let wifi = profiles.iter().find(|profile| is_wifi(profile));
    let Some(profile) = wifi.or(profiles.first()) else {
        return CheckResult::new(
            CheckId::NetworkPrivate,
            CheckStatus::Warning,
            "Not connected to a network",
            "Connect to your home Wi-Fi. Casting itself uses Wi-Fi Direct, but Windows only \
             allows the receiver on networks you trust.",
        );
    };
    if profile.category.eq_ignore_ascii_case("Public") {
        return CheckResult::new(
            CheckId::NetworkPrivate,
            CheckStatus::Problem,
            "Network is Public",
            format!(
                "\"{}\" is set to Public, so BudsLink keeps the receiver off. If this is your \
                 home network, make it Private. Never do this on cafe or airport Wi-Fi.",
                profile.name
            ),
        )
        .with_fix(Fix::MakeNetworkPrivate {
            network: profile.name.clone(),
        });
    }
    CheckResult::new(
        CheckId::NetworkPrivate,
        CheckStatus::Ok,
        "Network is Private",
        format!("\"{}\" ({})", profile.name, profile.category),
    )
}

/// `private_enabled`: Windows Firewall is on for Private networks;
/// `rule_present`: BudsLink's own rule exists.
pub fn firewall_check(private_enabled: Option<bool>, rule_present: bool) -> CheckResult {
    if rule_present {
        return CheckResult::new(
            CheckId::Firewall,
            CheckStatus::Ok,
            "Firewall allows BudsLink on Private networks",
            "",
        );
    }
    if private_enabled == Some(false) {
        return CheckResult::new(
            CheckId::Firewall,
            CheckStatus::Ok,
            "Firewall is off for Private networks",
            "Nothing blocks BudsLink. Adding the rule anyway keeps working if you turn it on.",
        )
        .with_fix(Fix::AddFirewallRule);
    }
    CheckResult::new(
        CheckId::Firewall,
        CheckStatus::Warning,
        "Firewall rule for BudsLink missing",
        "Allow BudsLink on Private networks only, so the phone's cast can reach it.",
    )
    .with_fix(Fix::AddFirewallRule)
}

/// What `netsh wlan show wirelesscapabilities` says about Miracast.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WifiCapabilities {
    pub miracast_sink: Option<bool>,
    pub wifi_direct_go: Option<bool>,
}

/// Parses `netsh wlan show wirelesscapabilities` (English Windows; other
/// languages leave the values unknown).
pub fn parse_wireless_capabilities(text: &str) -> WifiCapabilities {
    let mut caps = WifiCapabilities::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let supported = match value.trim().to_lowercase().as_str() {
            "supported" => Some(true),
            "not supported" => Some(false),
            _ => None,
        };
        match key.trim().to_lowercase().as_str() {
            "miracast sink" => caps.miracast_sink = supported,
            "wi-fi direct go" => caps.wifi_direct_go = supported,
            _ => {}
        }
    }
    caps
}

pub fn miracast_check(caps: WifiCapabilities) -> CheckResult {
    match caps.miracast_sink {
        Some(true) => CheckResult::new(
            CheckId::Miracast,
            CheckStatus::Ok,
            "Wi-Fi card can receive Miracast",
            "The phone can cast to this laptop.",
        ),
        Some(false) => CheckResult::new(
            CheckId::Miracast,
            CheckStatus::Problem,
            "This laptop's Wi-Fi card cannot receive Miracast",
            "Casting over Wi-Fi won't work here. BudsLink uses Bluetooth for the phone instead.",
        ),
        None => CheckResult::new(
            CheckId::Miracast,
            CheckStatus::Unknown,
            "Miracast support unknown",
            "Windows did not report it (Wi-Fi off, or a non-English Windows). Try casting; \
             if the phone never sees BudsLink, use Bluetooth.",
        ),
    }
}

pub fn wireless_display_check(installed: Option<bool>) -> CheckResult {
    match installed {
        Some(true) => CheckResult::new(
            CheckId::WirelessDisplay,
            CheckStatus::Ok,
            "Wireless Display installed",
            "Windows' own receiver is available as a fallback.",
        ),
        _ => CheckResult::new(
            CheckId::WirelessDisplay,
            CheckStatus::Optional,
            "Wireless Display (fallback receiver)",
            "Only needed if BudsLink's built-in receiver doesn't work on this PC. Installing \
             needs admin rights and internet once.",
        )
        .with_fix(Fix::InstallWirelessDisplay),
    }
}

/// What the PowerShell probe reports (see `setup.rs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PowerShellReport {
    pub networks: Vec<NetworkProfile>,
    pub firewall_private: Option<bool>,
    pub rule_present: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawReport {
    #[serde(default)]
    networks: Option<serde_json::Value>,
    #[serde(default)]
    firewall_private: Option<String>,
    #[serde(default)]
    rule_count: Option<u32>,
}

/// Parses `{ "Networks": [...], "FirewallPrivate": "True", "RuleCount": 1 }`.
pub fn parse_powershell_report(json: &str) -> PowerShellReport {
    let Ok(raw) = serde_json::from_str::<RawReport>(json.trim()) else {
        return PowerShellReport::default();
    };
    PowerShellReport {
        networks: raw
            .networks
            .map(|value| parse_network_profiles(&value.to_string()))
            .unwrap_or_default(),
        firewall_private: match raw.firewall_private.as_deref() {
            Some("True") => Some(true),
            Some("False") => Some(false),
            _ => None,
        },
        rule_present: raw.rule_count.unwrap_or(0) > 0,
    }
}

/// The admin script, embedded so nothing on disk can be swapped for it.
pub const FIX_SCRIPT: &str = include_str!("../../../scripts/fix-prereqs.ps1");

/// The parts of [`FIX_SCRIPT`] a fix runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixTask {
    Network,
    Firewall,
    WirelessDisplay,
}

impl FixTask {
    fn name(self) -> &'static str {
        match self {
            FixTask::Network => "network",
            FixTask::Firewall => "firewall",
            FixTask::WirelessDisplay => "wirelessdisplay",
        }
    }
}

/// A PowerShell single-quoted string literal.
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// The PowerShell text that runs [`FIX_SCRIPT`] with these arguments.
pub fn fix_invocation(tasks: &[FixTask], network: &str, program: &str) -> String {
    let tasks: Vec<&str> = tasks.iter().map(|task| task.name()).collect();
    format!(
        "& {{\n{FIX_SCRIPT}\n}} -Tasks {} -Network {} -Program {}",
        quoted(&tasks.join(",")),
        quoted(network),
        quoted(program)
    )
}

/// Base64 of the UTF-16LE text, as `powershell -EncodedCommand` expects.
pub fn encode_command(script: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> shift) as usize & 63]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_powershell_report() {
        let report = parse_powershell_report(
            r#"{"Networks":[{"Name":"Ali Hamza","InterfaceAlias":"Wi-Fi","Category":"Public"}],"FirewallPrivate":"True","RuleCount":0}"#,
        );
        assert_eq!(report.networks.len(), 1);
        assert_eq!(report.firewall_private, Some(true));
        assert!(!report.rule_present);
        assert_eq!(
            parse_powershell_report("not json"),
            PowerShellReport::default()
        );
        let report =
            parse_powershell_report(r#"{"FirewallPrivate":"NotConfigured","RuleCount":2}"#);
        assert_eq!(report.firewall_private, None);
        assert!(report.rule_present);
    }

    #[test]
    fn base64_matches_known_values() {
        // "a" in UTF-16LE is 61 00.
        assert_eq!(encode_command("a"), "YQA=");
        assert_eq!(encode_command("ab"), "YQBiAA==");
        assert_eq!(encode_command("abc"), "YQBiAGMA");
        assert_eq!(encode_command(""), "");
    }

    #[test]
    fn invocation_quotes_every_argument() {
        let text = fix_invocation(
            &[FixTask::Network, FixTask::Firewall],
            "Ali's Wi-Fi",
            r"C:\Users\Lenovo\AppData\Local\BudsLink\budslink.exe",
        );
        assert!(text.starts_with("& {\n<#"));
        assert!(text.ends_with(
            r"} -Tasks 'network,firewall' -Network 'Ali''s Wi-Fi' -Program 'C:\Users\Lenovo\AppData\Local\BudsLink\budslink.exe'"
        ));
    }

    const CAPABILITIES: &str = "
Wireless System Capabilities
----------------------------
    Number of antennas connected to the Wi-Fi radio (value not available)

Wireless Device Capabilities
----------------------------
    Wi-Fi Direct Device                         : Supported
    Wi-Fi Direct GO                             : Supported
    P2P GO on 6 GHz                             : Not Supported
    Miracast Sink                               : Supported
";

    #[test]
    fn parses_miracast_sink_support() {
        let caps = parse_wireless_capabilities(CAPABILITIES);
        assert_eq!(caps.miracast_sink, Some(true));
        assert_eq!(caps.wifi_direct_go, Some(true));
        assert_eq!(miracast_check(caps).status, CheckStatus::Ok);
    }

    #[test]
    fn unsupported_and_unknown_miracast() {
        let caps = parse_wireless_capabilities("    Miracast Sink      : Not Supported\n");
        assert_eq!(miracast_check(caps).status, CheckStatus::Problem);
        let caps = parse_wireless_capabilities("Empfänger    : Unterstützt\n");
        assert_eq!(caps.miracast_sink, None);
        assert_eq!(miracast_check(caps).status, CheckStatus::Unknown);
    }

    #[test]
    fn parses_one_or_many_network_profiles() {
        let one = r#"{ "Name": "Ali Hamza", "InterfaceAlias": "Wi-Fi", "Category": "Public" }"#;
        assert_eq!(parse_network_profiles(one).len(), 1);
        let many = r#"[
            { "Name": "Office", "InterfaceAlias": "Ethernet", "Category": "DomainAuthenticated" },
            { "Name": "Home", "InterfaceAlias": "Wi-Fi", "Category": "Private" }
        ]"#;
        assert_eq!(parse_network_profiles(many).len(), 2);
        assert!(parse_network_profiles("").is_empty());
        assert!(parse_network_profiles("garbage").is_empty());
    }

    #[test]
    fn public_wifi_offers_to_make_it_private() {
        let profiles = parse_network_profiles(
            r#"{ "Name": "Ali Hamza", "InterfaceAlias": "Wi-Fi", "Category": "Public" }"#,
        );
        let check = network_check(&profiles);
        assert_eq!(check.status, CheckStatus::Problem);
        assert_eq!(
            check.fix,
            Some(Fix::MakeNetworkPrivate {
                network: "Ali Hamza".into()
            })
        );
    }

    #[test]
    fn the_wifi_connection_decides_over_ethernet() {
        let profiles = parse_network_profiles(
            r#"[
            { "Name": "Office", "InterfaceAlias": "Ethernet", "Category": "Public" },
            { "Name": "Home", "InterfaceAlias": "Wi-Fi", "Category": "Private" }
        ]"#,
        );
        assert_eq!(network_check(&profiles).status, CheckStatus::Ok);
    }

    #[test]
    fn no_network_is_a_warning() {
        assert_eq!(network_check(&[]).status, CheckStatus::Warning);
    }

    #[test]
    fn firewall_states() {
        assert_eq!(firewall_check(Some(true), true).status, CheckStatus::Ok);
        assert_eq!(firewall_check(Some(true), true).fix, None);
        let missing = firewall_check(Some(true), false);
        assert_eq!(missing.status, CheckStatus::Warning);
        assert_eq!(missing.fix, Some(Fix::AddFirewallRule));
        assert_eq!(firewall_check(Some(false), false).status, CheckStatus::Ok);
    }

    #[test]
    fn bluetooth_states() {
        assert_eq!(bluetooth_check(Some(true)).status, CheckStatus::Ok);
        assert_eq!(bluetooth_check(Some(false)).status, CheckStatus::Problem);
        assert!(bluetooth_check(None).fix.is_some());
    }

    #[test]
    fn pairing_states() {
        assert_eq!(buds_paired_check(&[]).status, CheckStatus::Problem);
        assert_eq!(buds_paired_check(&["TWS".into()]).status, CheckStatus::Ok);
        assert_eq!(
            phone_paired_check(&["Infinix".into()], &[]).status,
            CheckStatus::Ok
        );
        let unavailable = phone_paired_check(&[], &["Infinix".into()]);
        assert_eq!(unavailable.status, CheckStatus::Warning);
        assert!(unavailable.detail.contains("pair it again"));
        assert_eq!(phone_paired_check(&[], &[]).status, CheckStatus::Warning);
    }

    #[test]
    fn wireless_display_is_optional() {
        assert_eq!(wireless_display_check(None).status, CheckStatus::Optional);
        assert_eq!(wireless_display_check(Some(true)).status, CheckStatus::Ok);
    }

    #[test]
    fn fixes_serialize_with_a_kind_tag() {
        let json = serde_json::to_string(&Fix::MakeNetworkPrivate {
            network: "Home".into(),
        })
        .unwrap();
        assert_eq!(json, r#"{"kind":"makeNetworkPrivate","network":"Home"}"#);
        let back: Fix = serde_json::from_str(r#"{"kind":"addFirewallRule"}"#).unwrap();
        assert_eq!(back, Fix::AddFirewallRule);
    }
}
