//! Turns the Miracast receiver's status codes into one verdict.
//!
//! Windows reports the receiver's health in several places: the Wi-Fi
//! support level, the result of applying settings, the result of starting a
//! session and the listening status. This module mirrors those codes as plain
//! Rust enums (so it compiles everywhere) and decides what they mean for the
//! user.

use std::fmt;

/// `MiracastReceiverWiFiStatus`: can this Wi-Fi card receive Miracast?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiSupport {
    Undetermined,
    NotSupported,
    NotOptimized,
    Supported,
}

/// `MiracastReceiverApplySettingsStatus` and
/// `MiracastReceiverSessionStartStatus` share these values (settings adds a
/// few "too long" cases, folded into `InvalidSettings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    UnknownFailure,
    MiracastNotSupported,
    AccessDenied,
    InvalidSettings,
}

/// `MiracastReceiverListeningStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listening {
    NotListening,
    Listening,
    ConnectionPending,
    Connected,
    DisabledByPolicy,
    TemporarilyDisabled,
}

impl Listening {
    /// True while a phone can find the receiver or is connected to it.
    pub fn is_ready(self) -> bool {
        matches!(
            self,
            Listening::Listening | Listening::ConnectionPending | Listening::Connected
        )
    }
}

/// What each step reported. `None` means the step did not run (an earlier
/// step failed) or the value could not be read.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProbeResult {
    pub wifi: Option<WifiSupport>,
    pub settings: Option<Outcome>,
    pub start: Option<Outcome>,
    pub listening: Option<Listening>,
}

/// The one thing the user needs to know about the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Listening: the phone can cast to BudsLink now.
    Ready,
    /// The Wi-Fi card or its driver cannot act as a Miracast receiver.
    WifiCardCannotReceive,
    /// Windows refused the receiver to this app. For an unpackaged app this
    /// most likely means the API needs package identity (plan B/C).
    AccessDenied,
    /// A group policy or MDM setting turned "Projecting to this PC" off.
    DisabledByPolicy,
    /// Windows paused the receiver, for example on a network it does not
    /// trust or while the PC is on battery with "only when plugged in".
    TemporarilyDisabled,
    /// Started, but Windows did not report a known state.
    Unknown,
}

/// Decides the verdict. Hard failures win over softer states, in the order
/// a user can act on them.
pub fn verdict(result: &ProbeResult) -> Verdict {
    let outcomes = [result.settings, result.start];
    if outcomes.contains(&Some(Outcome::AccessDenied)) {
        return Verdict::AccessDenied;
    }
    if result.wifi == Some(WifiSupport::NotSupported)
        || outcomes.contains(&Some(Outcome::MiracastNotSupported))
    {
        return Verdict::WifiCardCannotReceive;
    }
    match result.listening {
        Some(Listening::DisabledByPolicy) => Verdict::DisabledByPolicy,
        Some(Listening::TemporarilyDisabled) => Verdict::TemporarilyDisabled,
        Some(listening) if listening.is_ready() && result.start == Some(Outcome::Success) => {
            Verdict::Ready
        }
        _ => Verdict::Unknown,
    }
}

impl Verdict {
    /// Plain-English explanation with the next step, for the UI and the CLI.
    pub fn explain(self) -> &'static str {
        match self {
            Verdict::Ready => "BudsLink is ready. On the phone, open Cast and tap \"BudsLink\".",
            Verdict::WifiCardCannotReceive => {
                "This laptop's Wi-Fi card cannot receive Miracast. The phone can still \
                 reach your buds over Bluetooth."
            }
            Verdict::AccessDenied => {
                "Windows refused the Miracast receiver to BudsLink. The built-in \
                 Wireless Display app can be used instead (one-time install)."
            }
            Verdict::DisabledByPolicy => {
                "Projecting to this PC is turned off by a policy on this computer."
            }
            Verdict::TemporarilyDisabled => {
                "Windows paused the receiver. Check that the network is Private and, \
                 if \"only when plugged in\" is on, that the charger is connected."
            }
            Verdict::Unknown => {
                "The receiver started but Windows did not say it is listening. \
                 Copy diagnostics and check the log."
            }
        }
    }
}

impl fmt::Display for ProbeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn show<T: fmt::Debug>(value: &Option<T>) -> String {
            value
                .as_ref()
                .map_or_else(|| "-".to_owned(), |v| format!("{v:?}"))
        }
        write!(
            f,
            "wifi={} settings={} start={} listening={}",
            show(&self.wifi),
            show(&self.settings),
            show(&self.start),
            show(&self.listening)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn started(listening: Listening) -> ProbeResult {
        ProbeResult {
            wifi: Some(WifiSupport::Supported),
            settings: Some(Outcome::Success),
            start: Some(Outcome::Success),
            listening: Some(listening),
        }
    }

    #[test]
    fn listening_after_start_is_ready() {
        assert_eq!(verdict(&started(Listening::Listening)), Verdict::Ready);
        assert_eq!(verdict(&started(Listening::Connected)), Verdict::Ready);
        assert_eq!(
            verdict(&started(Listening::ConnectionPending)),
            Verdict::Ready
        );
    }

    #[test]
    fn not_listening_after_start_is_unknown() {
        assert_eq!(verdict(&started(Listening::NotListening)), Verdict::Unknown);
    }

    #[test]
    fn access_denied_wins_over_everything() {
        let result = ProbeResult {
            wifi: Some(WifiSupport::NotSupported),
            start: Some(Outcome::AccessDenied),
            ..ProbeResult::default()
        };
        assert_eq!(verdict(&result), Verdict::AccessDenied);
        let result = ProbeResult {
            settings: Some(Outcome::AccessDenied),
            ..ProbeResult::default()
        };
        assert_eq!(verdict(&result), Verdict::AccessDenied);
    }

    #[test]
    fn unsupported_wifi_is_reported() {
        let result = ProbeResult {
            wifi: Some(WifiSupport::NotSupported),
            ..ProbeResult::default()
        };
        assert_eq!(verdict(&result), Verdict::WifiCardCannotReceive);
        let result = ProbeResult {
            wifi: Some(WifiSupport::Undetermined),
            start: Some(Outcome::MiracastNotSupported),
            ..ProbeResult::default()
        };
        assert_eq!(verdict(&result), Verdict::WifiCardCannotReceive);
    }

    #[test]
    fn not_optimized_wifi_can_still_be_ready() {
        let result = ProbeResult {
            wifi: Some(WifiSupport::NotOptimized),
            ..started(Listening::Listening)
        };
        assert_eq!(verdict(&result), Verdict::Ready);
    }

    #[test]
    fn policy_and_pause_states() {
        assert_eq!(
            verdict(&started(Listening::DisabledByPolicy)),
            Verdict::DisabledByPolicy
        );
        assert_eq!(
            verdict(&started(Listening::TemporarilyDisabled)),
            Verdict::TemporarilyDisabled
        );
    }

    #[test]
    fn listening_without_a_started_session_is_not_ready() {
        let result = ProbeResult {
            start: None,
            ..started(Listening::Listening)
        };
        assert_eq!(verdict(&result), Verdict::Unknown);
    }

    #[test]
    fn display_shows_missing_steps() {
        let result = ProbeResult {
            wifi: Some(WifiSupport::Supported),
            ..ProbeResult::default()
        };
        assert_eq!(
            result.to_string(),
            "wifi=Supported settings=- start=- listening=-"
        );
    }

    #[test]
    fn every_verdict_has_an_explanation() {
        for verdict in [
            Verdict::Ready,
            Verdict::WifiCardCannotReceive,
            Verdict::AccessDenied,
            Verdict::DisabledByPolicy,
            Verdict::TemporarilyDisabled,
            Verdict::Unknown,
        ] {
            assert!(!verdict.explain().is_empty());
        }
    }
}
