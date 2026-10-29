//! Bridge mode: the phone streams its audio to the laptop, which acts as a
//! Bluetooth speaker, so buds connected only to the laptop play the phone and
//! the laptop together. No switching, and nothing to install on the phone.
//!
//! Like [`crate::controller`], the [`BridgeController`] is pure: snapshots,
//! commands and time go in, [`BridgeAction`]s come out.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::backend::{BackendError, ChangeNotifier, Watch};
use crate::device::DeviceId;
use crate::reconnect::ReconnectPolicy;

/// A paired phone (or other device) that can stream audio to this laptop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSource {
    pub id: DeviceId,
    pub name: String,
    /// The phone's audio is currently playing through the laptop.
    pub streaming: bool,
}

impl AudioSource {
    pub fn new(id: impl Into<String>, name: impl Into<String>, streaming: bool) -> Self {
        Self {
            id: DeviceId::new(id),
            name: name.into(),
            streaming,
        }
    }
}

/// Why a paired phone can't stream to this laptop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnavailableReason {
    /// Paired, but the system offers no audio from it.
    NoAudioService,
    /// Paired over Bluetooth Low Energy only, which carries no phone audio.
    LowEnergyOnly,
}

/// A paired phone that can't stream to this laptop. Listed with the reason so
/// a phone never just goes missing from the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnavailableSource {
    pub name: String,
    pub reason: UnavailableReason,
}

impl UnavailableSource {
    pub fn new(name: impl Into<String>, reason: UnavailableReason) -> Self {
        Self {
            name: name.into(),
            reason,
        }
    }
}

/// Access to devices that can stream audio *to* this laptop.
pub trait SourceBackend: Send + Sync {
    fn name(&self) -> &'static str;

    fn list_sources(&self) -> Result<Vec<AudioSource>, BackendError>;

    /// Paired phones that can't stream to this laptop. Best effort: empty when
    /// the backend can't tell.
    fn unavailable(&self) -> Vec<UnavailableSource> {
        Vec::new()
    }

    /// What the system reports about phones, as plain text for bug reports.
    fn diagnostics(&self) -> String {
        String::new()
    }

    /// Starts receiving audio from the device. Blocks until the stream is up
    /// or the attempt has failed.
    fn open(&self, id: &DeviceId) -> Result<(), BackendError>;

    fn close(&self, id: &DeviceId) -> Result<(), BackendError>;

    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BridgeState {
    /// Not wanted.
    Off,
    /// Opening the stream.
    Connecting,
    /// Phone audio is playing through the laptop.
    Streaming,
    /// Wanted, but the phone isn't streaming; another attempt is scheduled.
    Waiting,
    /// Wanted, but held off while the phone's audio arrives another way
    /// (casting over Wi-Fi), so it never plays twice.
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeAction {
    Open(DeviceId),
    Close(DeviceId),
}

/// One phone as shown in the UI and the tray.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub id: DeviceId,
    pub name: String,
    pub enabled: bool,
    pub state: BridgeState,
    pub last_error: Option<String>,
}

/// Retries while the user wants a phone bridged: 5 s, 10 s, 20 s, 40 s, then
/// every 60 s for as long as it stays enabled (the phone may simply be away).
pub fn default_retry_policy() -> ReconnectPolicy {
    ReconnectPolicy {
        initial_delay_ms: 5_000,
        max_delay_ms: 60_000,
        max_attempts: u32::MAX,
    }
}

#[derive(Debug, Clone)]
struct Entry {
    source: AudioSource,
    state: BridgeState,
    failures: u32,
    retry_at: Option<Instant>,
    last_error: Option<String>,
}

#[derive(Debug)]
pub struct BridgeController {
    policy: ReconnectPolicy,
    /// Phones the user turned on; kept even while a phone is unpaired or away.
    enabled: BTreeSet<DeviceId>,
    entries: BTreeMap<DeviceId, Entry>,
    /// Every stream held off (see [`BridgeState::Paused`]).
    suspended: bool,
}

impl BridgeController {
    pub fn new(enabled: impl IntoIterator<Item = DeviceId>, policy: ReconnectPolicy) -> Self {
        Self {
            policy,
            enabled: enabled.into_iter().collect(),
            entries: BTreeMap::new(),
            suspended: false,
        }
    }

    /// Holds every phone stream off (closing open ones) or lets them come
    /// back. Used so a phone that casts over Wi-Fi isn't also heard over
    /// Bluetooth.
    pub fn set_suspended(&mut self, suspended: bool, now: Instant) -> Vec<BridgeAction> {
        if self.suspended == suspended {
            return Vec::new();
        }
        self.suspended = suspended;
        let mut actions = Vec::new();
        for (id, entry) in &mut self.entries {
            if !self.enabled.contains(id) {
                continue;
            }
            entry.failures = 0;
            if suspended {
                if entry.source.streaming || entry.state == BridgeState::Connecting {
                    actions.push(BridgeAction::Close(id.clone()));
                }
                entry.source.streaming = false;
                entry.state = BridgeState::Paused;
                entry.retry_at = None;
            } else {
                entry.state = BridgeState::Waiting;
                entry.retry_at = Some(now);
            }
        }
        actions
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended
    }

    pub fn enabled(&self) -> Vec<DeviceId> {
        self.enabled.iter().cloned().collect()
    }

    pub fn apply_snapshot(&mut self, sources: Vec<AudioSource>, now: Instant) {
        let mut seen = BTreeSet::new();
        for source in sources {
            seen.insert(source.id.clone());
            let wanted = self.enabled.contains(&source.id);
            match self.entries.get_mut(&source.id) {
                Some(entry) => {
                    entry.source = source;
                    Self::observe(entry, wanted && !self.suspended, now, &self.policy);
                    if wanted && self.suspended && entry.state == BridgeState::Off {
                        entry.state = BridgeState::Paused;
                    }
                }
                None => {
                    let mut entry = Entry {
                        state: BridgeState::Off,
                        failures: 0,
                        retry_at: None,
                        last_error: None,
                        source,
                    };
                    if entry.source.streaming {
                        entry.state = BridgeState::Streaming;
                    } else if wanted && self.suspended {
                        entry.state = BridgeState::Paused;
                    } else if wanted {
                        // Turned on in an earlier session: try right away.
                        entry.state = BridgeState::Waiting;
                        entry.retry_at = Some(now);
                    }
                    self.entries.insert(entry.source.id.clone(), entry);
                }
            }
        }
        self.entries.retain(|id, _| seen.contains(id));
    }

    fn observe(entry: &mut Entry, wanted: bool, now: Instant, policy: &ReconnectPolicy) {
        match (entry.state, entry.source.streaming) {
            (_, true) if entry.state != BridgeState::Streaming => {
                entry.state = BridgeState::Streaming;
                entry.failures = 0;
                entry.retry_at = None;
                entry.last_error = None;
            }
            (BridgeState::Streaming, false) => {
                if wanted {
                    // The phone went away or stopped; keep trying.
                    entry.state = BridgeState::Waiting;
                    entry.failures = 1;
                    entry.retry_at = policy.delay_for_attempt(1).map(|delay| now + delay);
                } else {
                    entry.state = BridgeState::Off;
                }
            }
            _ => {}
        }
    }

    /// Turns bridging of one phone on or off.
    pub fn set_enabled(&mut self, id: &DeviceId, enabled: bool, now: Instant) -> Vec<BridgeAction> {
        if enabled {
            self.enabled.insert(id.clone());
        } else {
            self.enabled.remove(id);
        }
        let Some(entry) = self.entries.get_mut(id) else {
            return Vec::new();
        };
        entry.failures = 0;
        entry.last_error = None;
        if enabled {
            if self.suspended && !entry.source.streaming {
                entry.state = BridgeState::Paused;
                entry.retry_at = None;
            } else if !entry.source.streaming && entry.state != BridgeState::Connecting {
                entry.state = BridgeState::Waiting;
                entry.retry_at = Some(now);
            }
            Vec::new()
        } else {
            entry.retry_at = None;
            let was_active = entry.source.streaming || entry.state == BridgeState::Connecting;
            entry.state = BridgeState::Off;
            if was_active {
                vec![BridgeAction::Close(id.clone())]
            } else {
                Vec::new()
            }
        }
    }

    /// Starts due attempts.
    pub fn tick(&mut self, now: Instant) -> Vec<BridgeAction> {
        let mut actions = Vec::new();
        if self.suspended {
            return actions;
        }
        for (id, entry) in &mut self.entries {
            let due = entry.retry_at.is_some_and(|at| now >= at);
            if due && self.enabled.contains(id) && !entry.source.streaming {
                entry.retry_at = None;
                entry.state = BridgeState::Connecting;
                actions.push(BridgeAction::Open(id.clone()));
            }
        }
        actions
    }

    /// Records how an `Open` went.
    pub fn open_finished(&mut self, id: &DeviceId, result: Result<(), String>, now: Instant) {
        let Some(entry) = self.entries.get_mut(id) else {
            return;
        };
        if entry.state != BridgeState::Connecting {
            return; // turned off meanwhile
        }
        match result {
            Ok(()) => {
                entry.source.streaming = true;
                entry.state = BridgeState::Streaming;
                entry.failures = 0;
                entry.last_error = None;
            }
            Err(error) => {
                entry.failures = entry.failures.saturating_add(1);
                entry.last_error = Some(error);
                entry.state = BridgeState::Waiting;
                entry.retry_at = self
                    .policy
                    .delay_for_attempt(entry.failures)
                    .map(|delay| now + delay);
                if entry.retry_at.is_none() {
                    entry.state = BridgeState::Off;
                }
            }
        }
    }

    pub fn next_wakeup(&self) -> Option<Instant> {
        self.entries
            .values()
            .filter_map(|entry| entry.retry_at)
            .min()
    }

    pub fn views(&self) -> Vec<SourceView> {
        let mut views: Vec<_> = self
            .entries
            .values()
            .map(|entry| SourceView {
                id: entry.source.id.clone(),
                name: entry.source.name.clone(),
                enabled: self.enabled.contains(&entry.source.id),
                state: entry.state,
                last_error: entry.last_error.clone(),
            })
            .collect();
        views.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        views
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const PHONE: &str = "phone";

    fn id() -> DeviceId {
        DeviceId::new(PHONE)
    }

    fn phone(streaming: bool) -> AudioSource {
        AudioSource::new(PHONE, "Pixel", streaming)
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn view(controller: &BridgeController) -> SourceView {
        controller.views().into_iter().next().expect("one source")
    }

    #[test]
    fn new_phone_starts_off() {
        let start = Instant::now();
        let mut c = BridgeController::new([], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        assert_eq!(view(&c).state, BridgeState::Off);
        assert!(!view(&c).enabled);
        assert!(c.tick(start + secs(600)).is_empty());
        assert_eq!(c.next_wakeup(), None);
    }

    #[test]
    fn enabling_opens_and_streams() {
        let start = Instant::now();
        let mut c = BridgeController::new([], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);

        assert!(c.set_enabled(&id(), true, start).is_empty());
        assert_eq!(view(&c).state, BridgeState::Waiting);
        assert_eq!(c.tick(start), vec![BridgeAction::Open(id())]);
        assert_eq!(view(&c).state, BridgeState::Connecting);

        c.open_finished(&id(), Ok(()), start + secs(2));
        let v = view(&c);
        assert_eq!(v.state, BridgeState::Streaming);
        assert!(v.enabled);
        assert_eq!(c.enabled(), vec![id()]);
    }

    #[test]
    fn enabled_phone_from_last_session_opens_immediately() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        assert_eq!(c.tick(start), vec![BridgeAction::Open(id())]);
    }

    #[test]
    fn suspending_closes_the_stream_and_holds_it_off() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(true)], start);
        assert_eq!(view(&c).state, BridgeState::Streaming);

        assert_eq!(
            c.set_suspended(true, start),
            vec![BridgeAction::Close(id())]
        );
        assert_eq!(view(&c).state, BridgeState::Paused);
        assert!(c.is_suspended());
        // The phone stops streaming; nothing is retried while suspended.
        c.apply_snapshot(vec![phone(false)], start + secs(1));
        assert_eq!(view(&c).state, BridgeState::Paused);
        assert!(c.tick(start + secs(600)).is_empty());
        assert_eq!(c.next_wakeup(), None);
    }

    #[test]
    fn resuming_reopens_right_away() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        c.set_suspended(true, start);
        assert!(c.set_suspended(false, start + secs(5)).is_empty());
        assert_eq!(view(&c).state, BridgeState::Waiting);
        assert_eq!(c.tick(start + secs(5)), vec![BridgeAction::Open(id())]);
    }

    #[test]
    fn suspending_leaves_phones_that_are_off_alone() {
        let start = Instant::now();
        let mut c = BridgeController::new([], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        assert!(c.set_suspended(true, start).is_empty());
        assert_eq!(view(&c).state, BridgeState::Off);
        // Enabling while suspended waits for the resume.
        c.set_enabled(&id(), true, start);
        assert_eq!(view(&c).state, BridgeState::Paused);
        assert!(c.tick(start + secs(60)).is_empty());
    }

    #[test]
    fn a_phone_seen_while_suspended_starts_paused() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.set_suspended(true, start);
        c.apply_snapshot(vec![phone(false)], start);
        assert_eq!(view(&c).state, BridgeState::Paused);
        assert_eq!(
            c.set_suspended(true, start),
            Vec::new(),
            "no change, no actions"
        );
    }

    #[test]
    fn failures_back_off_and_keep_trying() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        c.tick(start);

        c.open_finished(&id(), Err("phone didn't answer".into()), start + secs(10));
        let v = view(&c);
        assert_eq!(v.state, BridgeState::Waiting);
        assert_eq!(v.last_error.as_deref(), Some("phone didn't answer"));
        assert_eq!(c.next_wakeup(), Some(start + secs(15)));
        assert!(c.tick(start + secs(14)).is_empty());
        assert_eq!(c.tick(start + secs(15)), vec![BridgeAction::Open(id())]);

        c.open_finished(&id(), Err("again".into()), start + secs(20));
        assert_eq!(c.next_wakeup(), Some(start + secs(30)));

        // Many failures later the delay is capped at a minute, never given up.
        for _ in 0..20 {
            let at = c.next_wakeup().unwrap();
            assert_eq!(c.tick(at), vec![BridgeAction::Open(id())]);
            c.open_finished(&id(), Err("away".into()), at);
        }
        let last = c.next_wakeup().unwrap();
        c.tick(last);
        c.open_finished(&id(), Err("away".into()), last);
        assert_eq!(c.next_wakeup(), Some(last + secs(60)));
    }

    #[test]
    fn dropped_stream_is_retried() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(true)], start);
        assert_eq!(view(&c).state, BridgeState::Streaming);

        c.apply_snapshot(vec![phone(false)], start + secs(1));
        assert_eq!(view(&c).state, BridgeState::Waiting);
        assert_eq!(c.tick(start + secs(6)), vec![BridgeAction::Open(id())]);
    }

    #[test]
    fn disabling_closes_and_stops_retrying() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(true)], start);

        assert_eq!(
            c.set_enabled(&id(), false, start),
            vec![BridgeAction::Close(id())]
        );
        assert_eq!(view(&c).state, BridgeState::Off);
        c.apply_snapshot(vec![phone(false)], start + secs(1));
        assert_eq!(view(&c).state, BridgeState::Off);
        assert!(c.tick(start + secs(600)).is_empty());
        assert!(c.enabled().is_empty());
    }

    #[test]
    fn late_open_result_after_disable_is_ignored() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        c.tick(start);
        assert_eq!(
            c.set_enabled(&id(), false, start),
            vec![BridgeAction::Close(id())]
        );
        c.open_finished(&id(), Ok(()), start + secs(1));
        assert_eq!(view(&c).state, BridgeState::Off);
    }

    #[test]
    fn missing_phone_stays_enabled() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        c.apply_snapshot(Vec::new(), start + secs(1)); // unpaired or not listed
        assert!(c.views().is_empty());
        assert_eq!(c.enabled(), vec![id()]);
        c.apply_snapshot(vec![phone(false)], start + secs(2));
        assert_eq!(c.tick(start + secs(2)), vec![BridgeAction::Open(id())]);
    }

    #[test]
    fn external_stream_is_shown() {
        let start = Instant::now();
        let mut c = BridgeController::new([], default_retry_policy());
        c.apply_snapshot(vec![phone(false)], start);
        c.apply_snapshot(vec![phone(true)], start + secs(1));
        assert_eq!(view(&c).state, BridgeState::Streaming);
        c.apply_snapshot(vec![phone(false)], start + secs(2));
        assert_eq!(view(&c).state, BridgeState::Off);
    }

    #[test]
    fn views_serialize_in_camel_case() {
        let start = Instant::now();
        let mut c = BridgeController::new([id()], default_retry_policy());
        c.apply_snapshot(vec![phone(true)], start);
        assert_eq!(
            serde_json::to_value(c.views()).unwrap(),
            serde_json::json!([{
                "id": "phone",
                "name": "Pixel",
                "enabled": true,
                "state": "streaming",
                "lastError": null
            }])
        );
    }

    #[test]
    fn unavailable_phones_serialize_in_camel_case() {
        let phones = [
            UnavailableSource::new("Pixel", UnavailableReason::NoAudioService),
            UnavailableSource::new("Galaxy", UnavailableReason::LowEnergyOnly),
        ];
        assert_eq!(
            serde_json::to_value(phones).unwrap(),
            serde_json::json!([
                { "name": "Pixel", "reason": "noAudioService" },
                { "name": "Galaxy", "reason": "lowEnergyOnly" }
            ])
        );
    }
}
