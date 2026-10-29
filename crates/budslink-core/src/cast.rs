//! Phone audio over Wi-Fi: when the Miracast receiver runs, which phones may
//! cast without asking, and restarts after a failure.
//!
//! Like the other controllers, [`CastController`] is pure: events and time go
//! in, [`CastAction`]s come out.

use std::time::Instant;

use serde::Serialize;

use crate::reconnect::ReconnectPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReceiverState {
    /// Turned off in settings (or "Bluetooth only").
    Off,
    /// Held off because the network is not Private.
    NotPrivate,
    Starting,
    /// Phones can find "BudsLink" in their Cast list.
    Ready,
    /// This PC cannot receive Miracast; the phone uses Bluetooth.
    Unsupported,
    /// Starting failed; another attempt is scheduled.
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CastPhase {
    Idle,
    /// A phone that is not on the allowlist is casting; the user decides.
    NeedsApproval,
    Connecting,
    Playing,
}

/// What the UI shows about casting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CastView {
    pub receiver: ReceiverState,
    pub message: Option<String>,
    pub phone: Option<String>,
    pub phase: CastPhase,
    pub pin: Option<String>,
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastAction {
    StartReceiver,
    StopReceiver,
    /// Let the current phone's stream play.
    Play,
    /// Disconnect the current phone.
    Reject,
}

/// How starting the receiver went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartOutcome {
    Ready,
    /// This PC can't do it at all; retrying won't help.
    Unsupported(String),
    /// Might work later (e.g. Wi-Fi was off).
    Failed(String),
}

/// Retries a failed receiver start: 5 s, 10 s, ... then every minute.
pub fn receiver_retry_policy() -> ReconnectPolicy {
    ReconnectPolicy {
        initial_delay_ms: 5_000,
        max_delay_ms: 60_000,
        max_attempts: u32::MAX,
    }
}

fn same_phone(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

#[derive(Debug)]
pub struct CastController {
    enabled: bool,
    private: bool,
    receiver: ReceiverState,
    message: Option<String>,
    phone: Option<String>,
    phase: CastPhase,
    pin: Option<String>,
    allowed: Vec<String>,
    policy: ReconnectPolicy,
    failures: u32,
    retry_at: Option<Instant>,
}

impl CastController {
    pub fn new(enabled: bool, allowed: Vec<String>, policy: ReconnectPolicy) -> Self {
        Self {
            enabled,
            private: false,
            receiver: ReceiverState::Off,
            message: None,
            phone: None,
            phase: CastPhase::Idle,
            pin: None,
            allowed,
            policy,
            failures: 0,
            retry_at: None,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool, now: Instant) -> Vec<CastAction> {
        self.enabled = enabled;
        if enabled && self.receiver == ReceiverState::Unsupported {
            // Turning it on again is how the user asks for another try.
            self.receiver = ReceiverState::Off;
        }
        self.reconcile(now)
    }

    /// The connected network's profile changed (or was read for the first time).
    pub fn set_private(&mut self, private: bool, now: Instant) -> Vec<CastAction> {
        self.private = private;
        self.reconcile(now)
    }

    pub fn start_finished(&mut self, outcome: StartOutcome, now: Instant) -> Vec<CastAction> {
        if self.receiver != ReceiverState::Starting {
            return Vec::new(); // stopped meanwhile
        }
        match outcome {
            StartOutcome::Ready => {
                self.receiver = ReceiverState::Ready;
                self.message = None;
                self.failures = 0;
            }
            StartOutcome::Unsupported(message) => {
                self.receiver = ReceiverState::Unsupported;
                self.message = Some(message);
            }
            StartOutcome::Failed(message) => {
                self.receiver = ReceiverState::Failed;
                self.message = Some(message);
                self.failures = self.failures.saturating_add(1);
                self.retry_at = self
                    .policy
                    .delay_for_attempt(self.failures)
                    .map(|delay| now + delay);
            }
        }
        Vec::new()
    }

    /// The receiver stopped by itself (Windows paused it, Wi-Fi went away).
    pub fn receiver_lost(&mut self, message: String, now: Instant) -> Vec<CastAction> {
        if !matches!(
            self.receiver,
            ReceiverState::Ready | ReceiverState::Starting
        ) {
            return Vec::new();
        }
        self.end_cast();
        self.receiver = ReceiverState::Failed;
        self.message = Some(message);
        self.failures = 1;
        self.retry_at = self.policy.delay_for_attempt(1).map(|delay| now + delay);
        vec![CastAction::StopReceiver]
    }

    pub fn phone_connected(&mut self, name: String, pin: Option<String>) -> Vec<CastAction> {
        self.pin = pin;
        let allowed = self.is_allowed(&name);
        self.phone = Some(name);
        if allowed {
            self.phase = CastPhase::Connecting;
            vec![CastAction::Play]
        } else {
            self.phase = CastPhase::NeedsApproval;
            Vec::new()
        }
    }

    pub fn stream_started(&mut self) {
        if self.phase == CastPhase::Connecting {
            self.phase = CastPhase::Playing;
        }
    }

    pub fn phone_disconnected(&mut self) {
        self.end_cast();
    }

    /// Lets the waiting phone play; `remember` adds it to the allowlist.
    /// Returns the name to persist when it was added.
    pub fn approve(&mut self, remember: bool) -> (Vec<CastAction>, Option<String>) {
        if self.phase != CastPhase::NeedsApproval {
            return (Vec::new(), None);
        }
        self.phase = CastPhase::Connecting;
        let added = match (&self.phone, remember) {
            (Some(name), true) if !self.is_allowed(name) => {
                self.allowed.push(name.clone());
                Some(name.clone())
            }
            _ => None,
        };
        (vec![CastAction::Play], added)
    }

    pub fn reject(&mut self) -> Vec<CastAction> {
        if self.phase == CastPhase::Idle {
            return Vec::new();
        }
        self.end_cast();
        vec![CastAction::Reject]
    }

    /// Removes a phone from the allowlist; it will be asked about again.
    pub fn forget(&mut self, name: &str) -> bool {
        let before = self.allowed.len();
        self.allowed.retain(|allowed| !same_phone(allowed, name));
        self.allowed.len() != before
    }

    pub fn allowed(&self) -> &[String] {
        &self.allowed
    }

    /// True while a phone sends its audio to the cast (so the Bluetooth link
    /// to the same phone must stay off).
    pub fn casting(&self) -> bool {
        self.phase != CastPhase::Idle
    }

    pub fn tick(&mut self, now: Instant) -> Vec<CastAction> {
        self.reconcile(now)
    }

    pub fn next_wakeup(&self) -> Option<Instant> {
        self.retry_at
    }

    pub fn view(&self) -> CastView {
        CastView {
            receiver: self.receiver,
            message: self.message.clone(),
            phone: self.phone.clone(),
            phase: self.phase,
            pin: self.pin.clone(),
            allowed: self.allowed.clone(),
        }
    }

    fn is_allowed(&self, name: &str) -> bool {
        self.allowed.iter().any(|allowed| same_phone(allowed, name))
    }

    fn end_cast(&mut self) {
        self.phone = None;
        self.pin = None;
        self.phase = CastPhase::Idle;
    }

    /// Starts or stops the receiver to match what is wanted.
    fn reconcile(&mut self, now: Instant) -> Vec<CastAction> {
        let running = matches!(
            self.receiver,
            ReceiverState::Starting | ReceiverState::Ready
        );
        if !self.enabled || !self.private {
            self.retry_at = None;
            self.failures = 0;
            let idle = if self.enabled {
                ReceiverState::NotPrivate
            } else {
                ReceiverState::Off
            };
            if self.receiver != ReceiverState::Unsupported || !self.enabled {
                self.receiver = idle;
            }
            if !self.private && self.enabled {
                self.message = Some(
                    "The network is not Private, so the receiver is off. Mark your home Wi-Fi \
                     Private in the setup check."
                        .into(),
                );
            }
            if running {
                self.end_cast();
                return vec![CastAction::StopReceiver];
            }
            return Vec::new();
        }
        match self.receiver {
            ReceiverState::Off | ReceiverState::NotPrivate => {
                self.message = None;
                self.receiver = ReceiverState::Starting;
                vec![CastAction::StartReceiver]
            }
            ReceiverState::Failed if self.retry_at.is_some_and(|at| now >= at) => {
                self.retry_at = None;
                self.receiver = ReceiverState::Starting;
                vec![CastAction::StartReceiver]
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn ready(allowed: &[&str]) -> (CastController, Instant) {
        let now = Instant::now();
        let mut c = CastController::new(
            true,
            allowed.iter().map(|name| (*name).to_owned()).collect(),
            receiver_retry_policy(),
        );
        assert_eq!(c.set_private(true, now), vec![CastAction::StartReceiver]);
        c.start_finished(StartOutcome::Ready, now);
        assert_eq!(c.view().receiver, ReceiverState::Ready);
        (c, now)
    }

    #[test]
    fn waits_for_a_private_network() {
        let now = Instant::now();
        let mut c = CastController::new(true, Vec::new(), receiver_retry_policy());
        assert!(c.set_private(false, now).is_empty());
        assert_eq!(c.view().receiver, ReceiverState::NotPrivate);
        assert!(c.view().message.unwrap().contains("not Private"));
        assert_eq!(c.set_private(true, now), vec![CastAction::StartReceiver]);
        assert_eq!(c.view().receiver, ReceiverState::Starting);
    }

    #[test]
    fn going_public_stops_the_receiver_and_the_cast() {
        let (mut c, now) = ready(&["Infinix HOT 11 Play"]);
        c.phone_connected("Infinix HOT 11 Play".into(), None);
        assert_eq!(c.set_private(false, now), vec![CastAction::StopReceiver]);
        assert_eq!(c.view().phase, CastPhase::Idle);
        assert!(!c.casting());
    }

    #[test]
    fn turning_off_stops_it() {
        let (mut c, now) = ready(&[]);
        assert_eq!(c.set_enabled(false, now), vec![CastAction::StopReceiver]);
        assert_eq!(c.view().receiver, ReceiverState::Off);
        assert!(c.set_enabled(false, now).is_empty());
    }

    #[test]
    fn an_allowed_phone_plays_without_asking() {
        let (mut c, _) = ready(&["Infinix HOT 11 Play"]);
        assert_eq!(
            c.phone_connected("infinix hot 11 play ".into(), None),
            vec![CastAction::Play]
        );
        assert!(c.casting());
        c.stream_started();
        assert_eq!(c.view().phase, CastPhase::Playing);
        c.phone_disconnected();
        assert_eq!(c.view().phase, CastPhase::Idle);
        assert_eq!(c.view().phone, None);
    }

    #[test]
    fn a_new_phone_needs_approval_once() {
        let (mut c, _) = ready(&[]);
        assert!(
            c.phone_connected("Infinix".into(), Some("1234".into()))
                .is_empty()
        );
        assert_eq!(c.view().phase, CastPhase::NeedsApproval);
        assert_eq!(c.view().pin.as_deref(), Some("1234"));
        // Stream start does not skip the approval.
        c.stream_started();
        assert_eq!(c.view().phase, CastPhase::NeedsApproval);

        let (actions, added) = c.approve(true);
        assert_eq!(actions, vec![CastAction::Play]);
        assert_eq!(added.as_deref(), Some("Infinix"));
        c.phone_disconnected();
        assert_eq!(
            c.phone_connected("Infinix".into(), None),
            vec![CastAction::Play]
        );
    }

    #[test]
    fn approving_just_this_once_remembers_nothing() {
        let (mut c, _) = ready(&[]);
        c.phone_connected("Guest phone".into(), None);
        let (actions, added) = c.approve(false);
        assert_eq!(actions, vec![CastAction::Play]);
        assert_eq!(added, None);
        assert!(c.allowed().is_empty());
        assert_eq!(
            c.approve(true),
            (Vec::new(), None),
            "nothing waits any more"
        );
    }

    #[test]
    fn rejecting_disconnects() {
        let (mut c, _) = ready(&[]);
        c.phone_connected("Stranger".into(), None);
        assert_eq!(c.reject(), vec![CastAction::Reject]);
        assert_eq!(c.view().phase, CastPhase::Idle);
        assert!(c.reject().is_empty());
    }

    #[test]
    fn forgetting_asks_again_next_time() {
        let (mut c, _) = ready(&["Infinix"]);
        assert!(c.forget("INFINIX"));
        assert!(!c.forget("Infinix"));
        assert!(c.phone_connected("Infinix".into(), None).is_empty());
    }

    #[test]
    fn failed_starts_are_retried_with_backoff() {
        let now = Instant::now();
        let mut c = CastController::new(true, Vec::new(), receiver_retry_policy());
        c.set_private(true, now);
        c.start_finished(StartOutcome::Failed("Wi-Fi is off".into()), now);
        assert_eq!(c.view().receiver, ReceiverState::Failed);
        assert_eq!(c.next_wakeup(), Some(now + Duration::from_secs(5)));
        assert!(c.tick(now + Duration::from_secs(4)).is_empty());
        assert_eq!(
            c.tick(now + Duration::from_secs(5)),
            vec![CastAction::StartReceiver]
        );
        c.start_finished(
            StartOutcome::Failed("still off".into()),
            now + Duration::from_secs(5),
        );
        assert_eq!(c.next_wakeup(), Some(now + Duration::from_secs(15)));
    }

    #[test]
    fn unsupported_is_not_retried_until_turned_on_again() {
        let now = Instant::now();
        let mut c = CastController::new(true, Vec::new(), receiver_retry_policy());
        c.set_private(true, now);
        c.start_finished(StartOutcome::Unsupported("no Miracast".into()), now);
        assert!(c.tick(now + Duration::from_secs(3600)).is_empty());
        assert_eq!(c.next_wakeup(), None);
        c.set_enabled(false, now);
        assert_eq!(c.set_enabled(true, now), vec![CastAction::StartReceiver]);
    }

    #[test]
    fn a_lost_receiver_restarts() {
        let (mut c, now) = ready(&["Infinix"]);
        c.phone_connected("Infinix".into(), None);
        assert_eq!(
            c.receiver_lost("Wi-Fi turned off".into(), now),
            vec![CastAction::StopReceiver]
        );
        assert!(!c.casting());
        assert_eq!(
            c.tick(now + Duration::from_secs(5)),
            vec![CastAction::StartReceiver]
        );
    }

    #[test]
    fn a_late_start_result_after_stopping_is_ignored() {
        let now = Instant::now();
        let mut c = CastController::new(true, Vec::new(), receiver_retry_policy());
        c.set_private(true, now);
        c.set_enabled(false, now);
        c.start_finished(StartOutcome::Ready, now);
        assert_eq!(c.view().receiver, ReceiverState::Off);
    }
}
