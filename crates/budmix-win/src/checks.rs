//! The setup checks: what BudMix needs from this PC, how to tell from what
//! Windows tools print, and how each problem can be fixed.
//!
//! Pure logic, compiled on every OS so it is unit-tested everywhere. The
//! Windows probing that feeds it lives in `setup.rs`.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckId {
    Bluetooth,
    BudsPaired,
    PhonePaired,
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
    /// BudMix can't do this part until it is fixed.
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
    /// Lets the casting phone's sound through Windows Firewall, for BudMix
    /// only (needs admin, one prompt).
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
            "Turn Bluetooth on so BudMix can reach your buds and phone.",
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
    one_or_many(json)
}

/// ConvertTo-Json prints one item as an object and several as an array.
fn one_or_many<T: DeserializeOwned>(json: &str) -> Vec<T> {
    let json = json.trim();
    if json.is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<Vec<T>>(json)
        .or_else(|_| serde_json::from_str::<T>(json).map(|item| vec![item]))
        .unwrap_or_default()
}

/// One Windows Firewall rule for BudMix.exe, whoever made it: BudMix, or
/// Windows after its own prompt.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FirewallRule {
    pub direction: String,
    pub action: String,
    /// "Public", "Private, Public", "Any", ...
    pub profile: String,
    pub enabled: String,
}

impl FirewallRule {
    /// The rule decides about traffic coming in on a Public network.
    fn covers_cast_link(&self) -> bool {
        self.direction == "Inbound"
            && self.enabled == "True"
            && (self.profile == "Any" || self.profile.contains("Public"))
    }
}

/// Can a casting phone's sound reach BudMix? It arrives over a direct Wi-Fi
/// link that Windows treats as Public, so only that profile counts. `None`
/// when Windows did not say.
pub fn cast_sound_allowed(report: &PowerShellReport) -> Option<bool> {
    if !report.firewall_public? {
        return Some(true);
    }
    let mut rules = report.rules.iter().filter(|rule| rule.covers_cast_link());
    // A Block rule beats every Allow rule.
    let blocked = rules.clone().any(|rule| rule.action == "Block");
    Some(!blocked && rules.any(|rule| rule.action == "Allow"))
}

pub fn firewall_check(report: &PowerShellReport) -> CheckResult {
    match (cast_sound_allowed(report), report.firewall_public) {
        (Some(true), Some(false)) => CheckResult::new(
            CheckId::Firewall,
            CheckStatus::Ok,
            "Firewall is off for Public networks",
            "Nothing blocks the phone's sound. Adding the rule anyway keeps it working if you \
             turn the firewall on.",
        )
        .with_fix(Fix::AddFirewallRule),
        (Some(true), _) => CheckResult::new(
            CheckId::Firewall,
            CheckStatus::Ok,
            "Firewall lets the phone's sound through",
            "",
        ),
        (Some(false), _) => CheckResult::new(
            CheckId::Firewall,
            CheckStatus::Warning,
            "Firewall blocks the sound of a casting phone",
            "A cast would connect without sound. Allow adds a rule for BudMix only, for devices \
             on the same local network, also on public Wi-Fi. Bluetooth doesn't need this.",
        )
        .with_fix(Fix::AddFirewallRule),
        (None, _) => CheckResult::new(
            CheckId::Firewall,
            CheckStatus::Unknown,
            "Firewall state unknown",
            "Windows did not report it. If a cast connects without sound, press Allow.",
        )
        .with_fix(Fix::AddFirewallRule),
    }
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
            "Casting over Wi-Fi won't work here. BudMix uses Bluetooth for the phone instead.",
        ),
        None => CheckResult::new(
            CheckId::Miracast,
            CheckStatus::Unknown,
            "Miracast support unknown",
            "Windows did not report it (Wi-Fi off, or a non-English Windows). Try casting; \
             if the phone never sees BudMix, use Bluetooth.",
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
            "Only needed if BudMix's built-in receiver doesn't work on this PC. Installing \
             needs admin rights and internet once.",
        )
        .with_fix(Fix::InstallWirelessDisplay),
    }
}

/// What the PowerShell probe reports (see `setup.rs`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PowerShellReport {
    pub networks: Vec<NetworkProfile>,
    /// Windows Firewall is on for Public networks.
    pub firewall_public: Option<bool>,
    /// Every firewall rule for BudMix.exe.
    pub rules: Vec<FirewallRule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawReport {
    #[serde(default)]
    networks: Option<serde_json::Value>,
    #[serde(default)]
    firewall_public: Option<String>,
    #[serde(default)]
    rules: Option<serde_json::Value>,
}

/// What the setup check asks PowerShell, for this BudMix.exe.
pub fn report_script(program: &str) -> String {
    const SCRIPT: &str = "\
$n = Get-NetConnectionProfile | Select-Object Name,InterfaceAlias,@{n='Category';e={\"$($_.NetworkCategory)\"}}
$f = (Get-NetFirewallProfile -Profile Public).Enabled
$r = Get-NetFirewallApplicationFilter -Program __PROGRAM__ -ErrorAction SilentlyContinue | Get-NetFirewallRule | Select-Object @{n='Direction';e={\"$($_.Direction)\"}},@{n='Action';e={\"$($_.Action)\"}},@{n='Profile';e={\"$($_.Profile)\"}},@{n='Enabled';e={\"$($_.Enabled)\"}}
[pscustomobject]@{ Networks = @($n); FirewallPublic = \"$f\"; Rules = @($r) } | ConvertTo-Json -Depth 4 -Compress";
    SCRIPT.replace("__PROGRAM__", &quoted(program))
}

/// Parses `{ "Networks": [...], "FirewallPublic": "True", "Rules": [...] }`.
pub fn parse_powershell_report(json: &str) -> PowerShellReport {
    let Ok(raw) = serde_json::from_str::<RawReport>(json.trim()) else {
        return PowerShellReport::default();
    };
    PowerShellReport {
        networks: raw
            .networks
            .map(|value| parse_network_profiles(&value.to_string()))
            .unwrap_or_default(),
        firewall_public: match raw.firewall_public.as_deref() {
            Some("True") => Some(true),
            Some("False") => Some(false),
            _ => None,
        },
        rules: raw
            .rules
            .map(|value| one_or_many(&value.to_string()))
            .unwrap_or_default(),
    }
}

/// The admin script, embedded so nothing on disk can be swapped for it.
pub const FIX_SCRIPT: &str = include_str!("../../../scripts/fix-prereqs.ps1");

/// The parts of [`FIX_SCRIPT`] a fix runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixTask {
    Firewall,
    WirelessDisplay,
}

impl FixTask {
    fn name(self) -> &'static str {
        match self {
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
pub fn fix_invocation(tasks: &[FixTask], program: &str) -> String {
    let tasks: Vec<&str> = tasks.iter().map(|task| task.name()).collect();
    format!(
        "& {{\n{FIX_SCRIPT}\n}} -Tasks {} -Program {}",
        quoted(&tasks.join(",")),
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
            r#"{"Networks":[{"Name":"Home Wi-Fi","InterfaceAlias":"Wi-Fi","Category":"Public"}],"FirewallPublic":"True","Rules":[]}"#,
        );
        assert_eq!(report.networks.len(), 1);
        assert_eq!(report.firewall_public, Some(true));
        assert!(report.rules.is_empty());
        assert_eq!(
            parse_powershell_report("not json"),
            PowerShellReport::default()
        );
        let report = parse_powershell_report(
            r#"{"FirewallPublic":"NotConfigured","Rules":{"Direction":"Inbound","Action":"Allow","Profile":"Public","Enabled":"True"}}"#,
        );
        assert_eq!(report.firewall_public, None);
        assert_eq!(
            report.rules,
            vec![rule("Inbound", "Allow", "Public", "True")]
        );
    }

    fn rule(direction: &str, action: &str, profile: &str, enabled: &str) -> FirewallRule {
        FirewallRule {
            direction: direction.into(),
            action: action.into(),
            profile: profile.into(),
            enabled: enabled.into(),
        }
    }

    fn firewall_on(rules: Vec<FirewallRule>) -> PowerShellReport {
        PowerShellReport {
            firewall_public: Some(true),
            rules,
            ..PowerShellReport::default()
        }
    }

    #[test]
    fn no_rule_blocks_the_cast_sound() {
        assert_eq!(cast_sound_allowed(&firewall_on(vec![])), Some(false));
    }

    #[test]
    fn a_rule_on_the_public_profile_lets_the_sound_through() {
        for profile in ["Public", "Private, Public", "Any"] {
            let report = firewall_on(vec![rule("Inbound", "Allow", profile, "True")]);
            assert_eq!(cast_sound_allowed(&report), Some(true), "{profile}");
        }
    }

    #[test]
    fn rules_that_do_not_reach_the_cast_link_do_not_count() {
        for rule in [
            rule("Inbound", "Allow", "Private", "True"),
            rule("Inbound", "Allow", "Public", "False"),
            rule("Outbound", "Allow", "Public", "True"),
        ] {
            assert_eq!(cast_sound_allowed(&firewall_on(vec![rule])), Some(false));
        }
    }

    #[test]
    fn a_block_rule_beats_an_allow_rule() {
        let report = firewall_on(vec![
            rule("Inbound", "Allow", "Public", "True"),
            rule("Inbound", "Block", "Public", "True"),
        ]);
        assert_eq!(cast_sound_allowed(&report), Some(false));
    }

    #[test]
    fn firewall_off_lets_everything_through() {
        let report = PowerShellReport {
            firewall_public: Some(false),
            rules: vec![rule("Inbound", "Block", "Public", "True")],
            ..PowerShellReport::default()
        };
        assert_eq!(cast_sound_allowed(&report), Some(true));
    }

    #[test]
    fn an_unknown_firewall_is_not_a_verdict() {
        assert_eq!(cast_sound_allowed(&PowerShellReport::default()), None);
        assert_eq!(
            firewall_check(&PowerShellReport::default()).status,
            CheckStatus::Unknown
        );
    }

    #[test]
    fn the_report_script_quotes_the_program() {
        let script = report_script(r"C:\Sam's apps\budmix.exe");
        assert!(script.contains(r"-Program 'C:\Sam''s apps\budmix.exe' -ErrorAction"));
        assert!(!script.contains("__PROGRAM__"));
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
            &[FixTask::Firewall, FixTask::WirelessDisplay],
            r"C:\Users\Sam's\AppData\Local\BudMix\budmix.exe",
        );
        assert!(text.starts_with("& {\n<#"));
        assert!(text.ends_with(
            r"} -Tasks 'firewall,wirelessdisplay' -Program 'C:\Users\Sam''s\AppData\Local\BudMix\budmix.exe'"
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
        let one = r#"{ "Name": "Home Wi-Fi", "InterfaceAlias": "Wi-Fi", "Category": "Public" }"#;
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
    fn firewall_states() {
        let allowed = firewall_check(&firewall_on(vec![rule(
            "Inbound", "Allow", "Public", "True",
        )]));
        assert_eq!(allowed.status, CheckStatus::Ok);
        assert_eq!(allowed.fix, None);
        let missing = firewall_check(&firewall_on(vec![]));
        assert_eq!(missing.status, CheckStatus::Warning);
        assert_eq!(missing.fix, Some(Fix::AddFirewallRule));
        let off = PowerShellReport {
            firewall_public: Some(false),
            ..PowerShellReport::default()
        };
        assert_eq!(firewall_check(&off).status, CheckStatus::Ok);
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
        let json = serde_json::to_string(&Fix::OpenSettings { uri: "x".into() }).unwrap();
        assert_eq!(json, r#"{"kind":"openSettings","uri":"x"}"#);
        let back: Fix = serde_json::from_str(r#"{"kind":"addFirewallRule"}"#).unwrap();
        assert_eq!(back, Fix::AddFirewallRule);
    }
}
