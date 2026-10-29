//! Pure connection logic.
//!
//! The [`Controller`] turns user commands, backend snapshots and the passage
//! of time into [`Action`]s. It never talks to the OS and never reads the
//! clock itself, so every transition is deterministic and unit-tested.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::device::{AudioDevice, AudioProfile, ConnectionState, DeviceId};
use crate::reconnect::ReconnectPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControllerConfig {
    pub auto_reconnect: bool,
    pub reconnect: ReconnectPolicy,
    pub connect_timeout: Duration,
    pub disconnect_timeout: Duration,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            auto_reconnect: true,
            reconnect: ReconnectPolicy::default(),
            connect_timeout: Duration::from_secs(15),
            disconnect_timeout: Duration::from_secs(10),
        }
    }
}

/// Something the user asked for (UI click, tray menu, hotkey).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Connect(DeviceId),
    Disconnect(DeviceId),
    Toggle(DeviceId),
}

impl Command {
    pub fn device_id(&self) -> &DeviceId {
        match self {
            Self::Connect(id) | Self::Disconnect(id) | Self::Toggle(id) => id,
        }
    }
}

/// A side effect the runtime must perform on the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Connect(DeviceId),
    Disconnect(DeviceId),
}

impl Action {
    pub fn device_id(&self) -> &DeviceId {
        match self {
            Self::Connect(id) | Self::Disconnect(id) => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ControllerError {
    #[error("unknown device {0}")]
    UnknownDevice(DeviceId),
}

/// One device as shown in the UI and the tray.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: DeviceId,
    pub name: String,
    pub profiles: Vec<AudioProfile>,
    pub state: ConnectionState,
    /// Auto-reconnect attempt that is scheduled or running, if any.
    pub reconnect_attempt: Option<u32>,
    pub last_error: Option<String>,
    /// The device connected most recently; it may still be connected.
    pub last_connected: bool,
}

pub const CONNECT_TIMEOUT_MESSAGE: &str =
    "Connection timed out. Are the buds out of the case and free from other devices?";
pub const DISCONNECT_TIMEOUT_MESSAGE: &str = "Disconnect timed out.";

/// What the user last asked for. Auto-reconnect only acts on `Connected`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    None,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone)]
struct Entry {
    device: AudioDevice,
    state: ConnectionState,
    intent: Intent,
    /// Timeout of the pending connect or disconnect.
    deadline: Option<Instant>,
    /// `Some(n)` while auto-reconnect attempt `n` is scheduled or running.
    reconnect_attempt: Option<u32>,
    retry_at: Option<Instant>,
    last_error: Option<String>,
}

impl Entry {
    fn new(device: AudioDevice) -> Self {
        let connected = device.connected;
        Self {
            device,
            state: if connected {
                ConnectionState::Connected
            } else {
                ConnectionState::Disconnected
            },
            // Buds that are already connected when BudsLink starts are
            // treated as wanted, so a later drop triggers auto-reconnect.
            intent: if connected {
                Intent::Connected
            } else {
                Intent::None
            },
            deadline: None,
            reconnect_attempt: None,
            retry_at: None,
            last_error: None,
        }
    }

    /// State the backend reports, ignoring anything in flight.
    fn settled_state(&self) -> ConnectionState {
        if self.device.connected {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        }
    }

    fn observe(&mut self, now: Instant, config: &ControllerConfig) {
        use ConnectionState::*;
        match (self.state, self.device.connected) {
            (Connecting | Disconnected, true) => {
                self.state = Connected;
                self.intent = Intent::Connected;
                self.deadline = None;
                self.last_error = None;
                self.clear_reconnect();
            }
            (Connected, false) => {
                self.state = Disconnected;
                if config.auto_reconnect && self.intent == Intent::Connected {
                    self.schedule_reconnect(1, now, config);
                }
            }
            (Disconnecting, false) => {
                self.state = Disconnected;
                self.deadline = None;
            }
            // Still waiting for a pending request, or nothing changed.
            (Connecting, false)
            | (Disconnecting, true)
            | (Connected, true)
            | (Disconnected, false) => {}
        }
    }

    fn schedule_reconnect(&mut self, attempt: u32, now: Instant, config: &ControllerConfig) {
        match config.reconnect.delay_for_attempt(attempt) {
            Some(delay) => {
                self.reconnect_attempt = Some(attempt);
                self.retry_at = Some(now + delay);
            }
            None => {
                if attempt > 1 {
                    self.last_error = Some(format!(
                        "Gave up reconnecting after {} attempts.",
                        attempt - 1
                    ));
                }
                self.clear_reconnect();
            }
        }
    }

    /// After a failed attempt, schedule the next one if a sequence is running.
    fn continue_reconnect(&mut self, now: Instant, config: &ControllerConfig) {
        match self.reconnect_attempt {
            Some(attempt) if config.auto_reconnect => {
                self.schedule_reconnect(attempt + 1, now, config)
            }
            _ => self.clear_reconnect(),
        }
    }

    fn clear_reconnect(&mut self) {
        self.reconnect_attempt = None;
        self.retry_at = None;
    }

    fn view(&self) -> DeviceView {
        DeviceView {
            id: self.device.id.clone(),
            name: self.device.name.clone(),
            profiles: self.device.profiles.clone(),
            state: self.state,
            reconnect_attempt: self.reconnect_attempt,
            last_error: self.last_error.clone(),
            last_connected: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct Controller {
    config: ControllerConfig,
    entries: BTreeMap<DeviceId, Entry>,
    /// So "the buds" still means the ones just released, after they are.
    last_connected: Option<DeviceId>,
}

impl Controller {
    pub fn new(config: ControllerConfig) -> Self {
        Self {
            config,
            entries: BTreeMap::new(),
            last_connected: None,
        }
    }

    pub fn config(&self) -> ControllerConfig {
        self.config
    }

    pub fn set_config(&mut self, config: ControllerConfig) {
        self.config = config;
        if !config.auto_reconnect {
            for entry in self.entries.values_mut() {
                entry.clear_reconnect();
            }
        }
    }

    /// Reconciles state with a fresh device list from the backend.
    pub fn apply_snapshot(&mut self, devices: Vec<AudioDevice>, now: Instant) {
        let mut seen = BTreeSet::new();
        for device in devices {
            seen.insert(device.id.clone());
            match self.entries.get_mut(&device.id) {
                Some(entry) => {
                    entry.device = device;
                    entry.observe(now, &self.config);
                }
                None => {
                    self.entries.insert(device.id.clone(), Entry::new(device));
                }
            }
        }
        self.entries.retain(|id, _| seen.contains(id));
        self.remember_connected();
    }

    /// Keeps `last_connected` on a connected device, preferring the one
    /// already remembered.
    fn remember_connected(&mut self) {
        let still_connected = self
            .last_connected
            .as_ref()
            .and_then(|id| self.entries.get(id))
            .is_some_and(|entry| entry.state.is_engaged());
        if !still_connected
            && let Some((id, _)) = self
                .entries
                .iter()
                .find(|(_, entry)| entry.state.is_engaged())
        {
            self.last_connected = Some(id.clone());
        }
    }

    pub fn handle_command(
        &mut self,
        command: Command,
        now: Instant,
    ) -> Result<Vec<Action>, ControllerError> {
        let id = command.device_id().clone();
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or_else(|| ControllerError::UnknownDevice(id.clone()))?;

        let want_connected = match command {
            Command::Connect(_) => true,
            Command::Disconnect(_) => false,
            Command::Toggle(_) => !entry.state.is_engaged(),
        };

        entry.clear_reconnect();
        entry.last_error = None;

        if want_connected {
            entry.intent = Intent::Connected;
            if entry.state.is_engaged() {
                return Ok(Vec::new());
            }
            entry.state = ConnectionState::Connecting;
            entry.deadline = Some(now + self.config.connect_timeout);
            self.last_connected = Some(id.clone());
            Ok(vec![Action::Connect(id)])
        } else {
            entry.intent = Intent::Disconnected;
            if !entry.state.is_engaged() {
                return Ok(Vec::new());
            }
            entry.state = ConnectionState::Disconnecting;
            entry.deadline = Some(now + self.config.disconnect_timeout);
            Ok(vec![Action::Disconnect(id)])
        }
    }

    /// Records that the backend rejected an action outright.
    pub fn action_failed(&mut self, action: &Action, error: impl Into<String>, now: Instant) {
        let Some(entry) = self.entries.get_mut(action.device_id()) else {
            return;
        };
        let expected = match action {
            Action::Connect(_) => ConnectionState::Connecting,
            Action::Disconnect(_) => ConnectionState::Disconnecting,
        };
        if entry.state != expected {
            return;
        }
        entry.last_error = Some(error.into());
        entry.deadline = None;
        entry.state = entry.settled_state();
        if entry.state == ConnectionState::Disconnected {
            entry.continue_reconnect(now, &self.config);
        }
    }

    /// Fires timeouts and due reconnect attempts.
    pub fn tick(&mut self, now: Instant) -> Vec<Action> {
        let mut actions = Vec::new();
        for (id, entry) in &mut self.entries {
            if entry.deadline.is_some_and(|deadline| now >= deadline) {
                entry.deadline = None;
                let pending = entry.state;
                entry.state = entry.settled_state();
                match (pending, entry.state) {
                    (ConnectionState::Connecting, ConnectionState::Disconnected) => {
                        entry.last_error = Some(CONNECT_TIMEOUT_MESSAGE.to_owned());
                        entry.continue_reconnect(now, &self.config);
                    }
                    (ConnectionState::Disconnecting, ConnectionState::Connected) => {
                        entry.last_error = Some(DISCONNECT_TIMEOUT_MESSAGE.to_owned());
                    }
                    _ => {}
                }
            }

            if entry.state == ConnectionState::Disconnected
                && entry.retry_at.is_some_and(|at| now >= at)
            {
                entry.retry_at = None;
                entry.state = ConnectionState::Connecting;
                entry.deadline = Some(now + self.config.connect_timeout);
                actions.push(Action::Connect(id.clone()));
            }
        }
        actions
    }

    /// Earliest instant at which [`Controller::tick`] has work to do.
    pub fn next_wakeup(&self) -> Option<Instant> {
        self.entries
            .values()
            .flat_map(|entry| [entry.deadline, entry.retry_at])
            .flatten()
            .min()
    }

    pub fn has_pending(&self) -> bool {
        self.entries.values().any(|entry| entry.state.is_pending())
    }

    /// Devices sorted by name, for the UI and tray.
    pub fn views(&self) -> Vec<DeviceView> {
        let mut views: Vec<_> = self
            .entries
            .values()
            .map(|entry| DeviceView {
                last_connected: self.last_connected.as_ref() == Some(&entry.device.id),
                ..entry.view()
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

    /// Device a "toggle" hotkey acts on: the preferred device if it is
    /// paired, otherwise the connected one, otherwise the one connected most
    /// recently, otherwise the first by name.
    pub fn hotkey_target(&self, preferred: Option<&DeviceId>) -> Option<DeviceId> {
        if let Some(id) = preferred.filter(|id| self.entries.contains_key(*id)) {
            return Some(id.clone());
        }
        let views = self.views();
        views
            .iter()
            .find(|view| view.state.is_engaged())
            .or_else(|| views.iter().find(|view| view.last_connected))
            .or_else(|| views.first())
            .map(|view| view.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDS: &str = "buds";
    const SPEAKER: &str = "speaker";

    fn id(raw: &str) -> DeviceId {
        DeviceId::new(raw)
    }

    fn device(raw: &str, connected: bool) -> AudioDevice {
        AudioDevice::new(raw, raw.to_uppercase(), connected)
    }

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    struct Harness {
        controller: Controller,
        start: Instant,
    }

    impl Harness {
        fn new(config: ControllerConfig) -> Self {
            Self {
                controller: Controller::new(config),
                start: Instant::now(),
            }
        }

        fn at(&self, offset: Duration) -> Instant {
            self.start + offset
        }

        fn snapshot(&mut self, offset: Duration, devices: Vec<AudioDevice>) {
            let now = self.at(offset);
            self.controller.apply_snapshot(devices, now);
        }

        fn command(&mut self, offset: Duration, command: Command) -> Vec<Action> {
            let now = self.at(offset);
            self.controller.handle_command(command, now).unwrap()
        }

        fn tick(&mut self, offset: Duration) -> Vec<Action> {
            let now = self.at(offset);
            self.controller.tick(now)
        }

        fn view(&self, raw: &str) -> DeviceView {
            self.controller
                .views()
                .into_iter()
                .find(|view| view.id.as_str() == raw)
                .expect("device present")
        }

        fn state(&self, raw: &str) -> ConnectionState {
            self.view(raw).state
        }
    }

    fn harness() -> Harness {
        Harness::new(ControllerConfig::default())
    }

    #[test]
    fn first_snapshot_reflects_backend_state() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true), device(SPEAKER, false)]);
        assert_eq!(h.state(BUDS), ConnectionState::Connected);
        assert_eq!(h.state(SPEAKER), ConnectionState::Disconnected);
        assert!(!h.controller.has_pending());
        assert_eq!(h.controller.next_wakeup(), None);
    }

    #[test]
    fn views_are_sorted_by_name() {
        let mut h = harness();
        h.snapshot(
            secs(0),
            vec![
                AudioDevice::new("2", "zeta", false),
                AudioDevice::new("1", "Alpha", false),
            ],
        );
        let names: Vec<_> = h.controller.views().into_iter().map(|v| v.name).collect();
        assert_eq!(names, ["Alpha", "zeta"]);
    }

    #[test]
    fn connect_goes_through_connecting() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, false)]);

        let actions = h.command(secs(1), Command::Connect(id(BUDS)));
        assert_eq!(actions, vec![Action::Connect(id(BUDS))]);
        assert_eq!(h.state(BUDS), ConnectionState::Connecting);
        assert!(h.controller.has_pending());

        // The backend may still report "disconnected" while the link comes up.
        h.snapshot(secs(2), vec![device(BUDS, false)]);
        assert_eq!(h.state(BUDS), ConnectionState::Connecting);

        h.snapshot(secs(3), vec![device(BUDS, true)]);
        assert_eq!(h.state(BUDS), ConnectionState::Connected);
        assert!(!h.controller.has_pending());
        assert_eq!(h.controller.next_wakeup(), None);
    }

    #[test]
    fn connect_when_already_connected_is_a_no_op() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        assert!(h.command(secs(1), Command::Connect(id(BUDS))).is_empty());
        assert_eq!(h.state(BUDS), ConnectionState::Connected);
    }

    #[test]
    fn unknown_device_is_an_error() {
        let mut h = harness();
        let result = h
            .controller
            .handle_command(Command::Connect(id("ghost")), h.start);
        assert_eq!(result, Err(ControllerError::UnknownDevice(id("ghost"))));
    }

    #[test]
    fn manual_disconnect_does_not_auto_reconnect() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);

        let actions = h.command(secs(1), Command::Disconnect(id(BUDS)));
        assert_eq!(actions, vec![Action::Disconnect(id(BUDS))]);
        assert_eq!(h.state(BUDS), ConnectionState::Disconnecting);

        h.snapshot(secs(2), vec![device(BUDS, false)]);
        assert_eq!(h.state(BUDS), ConnectionState::Disconnected);
        assert_eq!(h.view(BUDS).reconnect_attempt, None);
        assert!(h.tick(secs(120)).is_empty());
    }

    #[test]
    fn toggle_flips_between_connect_and_disconnect() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, false)]);
        assert_eq!(
            h.command(secs(1), Command::Toggle(id(BUDS))),
            vec![Action::Connect(id(BUDS))]
        );
        // Toggling while connecting cancels the connection.
        assert_eq!(
            h.command(secs(2), Command::Toggle(id(BUDS))),
            vec![Action::Disconnect(id(BUDS))]
        );
        assert_eq!(h.state(BUDS), ConnectionState::Disconnecting);
        // Toggling while disconnecting connects again.
        assert_eq!(
            h.command(secs(3), Command::Toggle(id(BUDS))),
            vec![Action::Connect(id(BUDS))]
        );
    }

    #[test]
    fn unexpected_drop_reconnects_with_back_off() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);

        // Buds drop (e.g. out of range).
        h.snapshot(secs(10), vec![device(BUDS, false)]);
        assert_eq!(h.state(BUDS), ConnectionState::Disconnected);
        assert_eq!(h.view(BUDS).reconnect_attempt, Some(1));
        assert_eq!(h.controller.next_wakeup(), Some(h.at(secs(11))));

        assert!(h.tick(Duration::from_millis(10_999)).is_empty());
        assert_eq!(h.tick(secs(11)), vec![Action::Connect(id(BUDS))]);
        assert_eq!(h.state(BUDS), ConnectionState::Connecting);

        // Attempt 1 times out after 15 s; attempt 2 waits 2 s more.
        assert!(h.tick(secs(26)).is_empty());
        assert_eq!(h.state(BUDS), ConnectionState::Disconnected);
        assert_eq!(h.view(BUDS).reconnect_attempt, Some(2));
        assert_eq!(
            h.view(BUDS).last_error.as_deref(),
            Some(CONNECT_TIMEOUT_MESSAGE)
        );
        assert_eq!(h.tick(secs(28)), vec![Action::Connect(id(BUDS))]);

        // Attempt 2 succeeds.
        h.snapshot(secs(30), vec![device(BUDS, true)]);
        let view = h.view(BUDS);
        assert_eq!(view.state, ConnectionState::Connected);
        assert_eq!(view.reconnect_attempt, None);
        assert_eq!(view.last_error, None);
    }

    #[test]
    fn gives_up_after_max_attempts() {
        let mut h = Harness::new(ControllerConfig {
            reconnect: ReconnectPolicy {
                initial_delay_ms: 1_000,
                max_delay_ms: 1_000,
                max_attempts: 2,
            },
            connect_timeout: secs(5),
            ..Default::default()
        });
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        h.snapshot(secs(1), vec![device(BUDS, false)]);

        assert_eq!(h.tick(secs(2)), vec![Action::Connect(id(BUDS))]); // attempt 1
        assert!(h.tick(secs(7)).is_empty()); // timeout, attempt 2 at 8 s
        assert_eq!(h.tick(secs(8)), vec![Action::Connect(id(BUDS))]); // attempt 2
        assert!(h.tick(secs(13)).is_empty()); // timeout, give up

        let view = h.view(BUDS);
        assert_eq!(view.state, ConnectionState::Disconnected);
        assert_eq!(view.reconnect_attempt, None);
        assert_eq!(
            view.last_error.as_deref(),
            Some("Gave up reconnecting after 2 attempts.")
        );
        assert_eq!(h.controller.next_wakeup(), None);
        assert!(h.tick(secs(600)).is_empty());
    }

    #[test]
    fn auto_reconnect_can_be_disabled() {
        let mut h = Harness::new(ControllerConfig {
            auto_reconnect: false,
            ..Default::default()
        });
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        h.snapshot(secs(1), vec![device(BUDS, false)]);
        assert_eq!(h.view(BUDS).reconnect_attempt, None);
        assert!(h.tick(secs(60)).is_empty());
    }

    #[test]
    fn disabling_auto_reconnect_cancels_scheduled_attempt() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        h.snapshot(secs(1), vec![device(BUDS, false)]);
        assert_eq!(h.view(BUDS).reconnect_attempt, Some(1));

        h.controller.set_config(ControllerConfig {
            auto_reconnect: false,
            ..Default::default()
        });
        assert_eq!(h.view(BUDS).reconnect_attempt, None);
        assert!(h.tick(secs(60)).is_empty());
    }

    #[test]
    fn manual_command_cancels_scheduled_attempt() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        h.snapshot(secs(1), vec![device(BUDS, false)]);
        // User hands the buds over to the phone.
        assert!(h.command(secs(1), Command::Disconnect(id(BUDS))).is_empty());
        assert_eq!(h.view(BUDS).reconnect_attempt, None);
        assert!(h.tick(secs(60)).is_empty());
    }

    #[test]
    fn manual_connect_timeout_does_not_retry() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, false)]);
        h.command(secs(0), Command::Connect(id(BUDS)));
        assert!(h.tick(secs(15)).is_empty());

        let view = h.view(BUDS);
        assert_eq!(view.state, ConnectionState::Disconnected);
        assert_eq!(view.reconnect_attempt, None);
        assert_eq!(view.last_error.as_deref(), Some(CONNECT_TIMEOUT_MESSAGE));
    }

    #[test]
    fn rejected_connect_reports_error() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, false)]);
        let actions = h.command(secs(0), Command::Connect(id(BUDS)));
        h.controller
            .action_failed(&actions[0], "driver refused", h.at(secs(0)));

        let view = h.view(BUDS);
        assert_eq!(view.state, ConnectionState::Disconnected);
        assert_eq!(view.last_error.as_deref(), Some("driver refused"));
        assert!(!h.controller.has_pending());
    }

    #[test]
    fn rejected_reconnect_schedules_next_attempt() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        h.snapshot(secs(1), vec![device(BUDS, false)]);
        let actions = h.tick(secs(2));
        h.controller
            .action_failed(&actions[0], "driver refused", h.at(secs(2)));

        assert_eq!(h.view(BUDS).reconnect_attempt, Some(2));
        assert_eq!(h.controller.next_wakeup(), Some(h.at(secs(4))));
    }

    #[test]
    fn stale_failure_is_ignored() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, false)]);
        let actions = h.command(secs(0), Command::Connect(id(BUDS)));
        h.snapshot(secs(1), vec![device(BUDS, true)]);
        h.controller
            .action_failed(&actions[0], "late error", h.at(secs(2)));
        assert_eq!(h.state(BUDS), ConnectionState::Connected);
        assert_eq!(h.view(BUDS).last_error, None);
    }

    #[test]
    fn disconnect_timeout_falls_back_to_connected() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        h.command(secs(0), Command::Disconnect(id(BUDS)));
        assert!(h.tick(secs(10)).is_empty());

        let view = h.view(BUDS);
        assert_eq!(view.state, ConnectionState::Connected);
        assert_eq!(view.last_error.as_deref(), Some(DISCONNECT_TIMEOUT_MESSAGE));
    }

    #[test]
    fn external_connect_is_accepted_and_wanted() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true)]);
        h.command(secs(0), Command::Disconnect(id(BUDS)));
        h.snapshot(secs(1), vec![device(BUDS, false)]);

        // The user reconnects from Windows Settings, then the buds drop.
        h.snapshot(secs(5), vec![device(BUDS, true)]);
        assert_eq!(h.state(BUDS), ConnectionState::Connected);
        h.snapshot(secs(6), vec![device(BUDS, false)]);
        assert_eq!(h.view(BUDS).reconnect_attempt, Some(1));
    }

    #[test]
    fn unpaired_devices_disappear() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true), device(SPEAKER, false)]);
        h.snapshot(secs(1), vec![device(SPEAKER, false)]);
        let ids: Vec<_> = h.controller.views().into_iter().map(|v| v.id).collect();
        assert_eq!(ids, vec![id(SPEAKER)]);
    }

    #[test]
    fn next_wakeup_is_the_earliest_deadline() {
        let mut h = harness();
        h.snapshot(secs(0), vec![device(BUDS, true), device(SPEAKER, false)]);
        h.snapshot(secs(1), vec![device(BUDS, false), device(SPEAKER, false)]); // retry at 2 s
        h.command(secs(1), Command::Connect(id(SPEAKER))); // timeout at 16 s
        assert_eq!(h.controller.next_wakeup(), Some(h.at(secs(2))));
    }

    #[test]
    fn hotkey_target_prefers_configured_then_connected_then_first() {
        let mut h = harness();
        h.snapshot(
            secs(0),
            vec![
                AudioDevice::new("a", "Alpha", false),
                AudioDevice::new("b", "Beta", true),
            ],
        );
        let c = &h.controller;
        assert_eq!(c.hotkey_target(Some(&id("a"))), Some(id("a")));
        assert_eq!(c.hotkey_target(Some(&id("missing"))), Some(id("b")));
        assert_eq!(c.hotkey_target(None), Some(id("b")));

        let mut fresh = harness();
        fresh.snapshot(
            secs(0),
            vec![
                AudioDevice::new("a", "Alpha", false),
                AudioDevice::new("b", "Beta", false),
            ],
        );
        assert_eq!(fresh.controller.hotkey_target(None), Some(id("a")));
        assert_eq!(Controller::default().hotkey_target(None), None);
    }

    #[test]
    fn released_buds_stay_the_hotkey_target() {
        let mut h = harness();
        h.snapshot(
            secs(0),
            vec![
                AudioDevice::new("a", "Alpha", false),
                AudioDevice::new("b", "Beta", true),
            ],
        );
        h.command(secs(1), Command::Disconnect(id("b")));
        h.snapshot(
            secs(2),
            vec![
                AudioDevice::new("a", "Alpha", false),
                AudioDevice::new("b", "Beta", false),
            ],
        );
        assert_eq!(h.controller.hotkey_target(None), Some(id("b")));
        let views = h.controller.views();
        assert!(!views[0].last_connected);
        assert!(views[1].last_connected);

        // Connecting another device makes that one "the buds".
        h.command(secs(3), Command::Connect(id("a")));
        assert_eq!(h.controller.hotkey_target(None), Some(id("a")));
    }
}
