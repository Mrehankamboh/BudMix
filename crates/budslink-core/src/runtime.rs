//! Imperative shell around the [`Controller`].
//!
//! A worker thread owns the controller, listens for backend change
//! notifications, fires timers and executes actions. Callers talk to it
//! through a cheap, cloneable [`RuntimeHandle`].

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::backend::{ChangeNotifier, DeviceBackend};
use crate::controller::{Action, Command, Controller, ControllerConfig, DeviceView};
use crate::device::DeviceId;

/// Re-check interval while a connect/disconnect is in flight.
const PENDING_POLL: Duration = Duration::from_secs(1);
/// Safety-net refresh in case a change notification is missed.
const IDLE_POLL: Duration = Duration::from_secs(30);
/// Refresh interval when the backend cannot send notifications.
const FALLBACK_POLL: Duration = Duration::from_secs(5);

enum Request {
    Command(Command),
    /// Toggle the hotkey target, given the preferred device from settings.
    ToggleTarget(Option<DeviceId>),
    Refresh,
    Configure(ControllerConfig),
    Shutdown,
}

/// Called with the full device list every time it changes.
pub type UpdateListener = Box<dyn Fn(&[DeviceView]) + Send>;

#[derive(Clone)]
pub struct RuntimeHandle {
    tx: Sender<Request>,
    views: Arc<Mutex<Vec<DeviceView>>>,
    backend_name: &'static str,
}

impl RuntimeHandle {
    pub fn send(&self, command: Command) {
        self.post(Request::Command(command));
    }

    /// Toggles the preferred device, or a sensible fallback (see
    /// [`Controller::hotkey_target`]).
    pub fn toggle_target(&self, preferred: Option<DeviceId>) {
        self.post(Request::ToggleTarget(preferred));
    }

    pub fn refresh(&self) {
        self.post(Request::Refresh);
    }

    pub fn configure(&self, config: ControllerConfig) {
        self.post(Request::Configure(config));
    }

    /// Latest published device list.
    pub fn views(&self) -> Vec<DeviceView> {
        self.views
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    fn post(&self, request: Request) {
        if self.tx.send(request).is_err() {
            log::warn!("BudsLink runtime has stopped; request dropped");
        }
    }
}

/// Owns the worker thread; dropping it stops the worker.
pub struct Runtime {
    handle: RuntimeHandle,
    worker: Option<JoinHandle<()>>,
}

impl Runtime {
    pub fn spawn(
        backend: Arc<dyn DeviceBackend>,
        config: ControllerConfig,
        on_update: UpdateListener,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let views = Arc::new(Mutex::new(Vec::new()));
        let handle = RuntimeHandle {
            tx: tx.clone(),
            views: Arc::clone(&views),
            backend_name: backend.name(),
        };
        let worker = thread::Builder::new()
            .name("budslink-runtime".into())
            .spawn(move || {
                Worker {
                    backend,
                    controller: Controller::new(config),
                    views,
                    on_update,
                    last_refresh: Instant::now(),
                }
                .run(tx, rx)
            })
            .expect("failed to spawn the BudsLink runtime thread");
        Self {
            handle,
            worker: Some(worker),
        }
    }

    pub fn handle(&self) -> RuntimeHandle {
        self.handle.clone()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.handle.tx.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Worker {
    backend: Arc<dyn DeviceBackend>,
    controller: Controller,
    views: Arc<Mutex<Vec<DeviceView>>>,
    on_update: UpdateListener,
    last_refresh: Instant,
}

impl Worker {
    fn run(mut self, tx: Sender<Request>, rx: Receiver<Request>) {
        let notify: ChangeNotifier = Arc::new(move || {
            let _ = tx.send(Request::Refresh);
        });
        let (_watch, poll) = match self.backend.watch(notify) {
            Ok(watch) => (Some(watch), IDLE_POLL),
            Err(err) => {
                log::warn!("change notifications unavailable ({err}); polling instead");
                (None, FALLBACK_POLL)
            }
        };

        self.refresh();
        self.publish();

        loop {
            let now = Instant::now();
            let mut timeout = poll.saturating_sub(now - self.last_refresh);
            if self.controller.has_pending() {
                timeout = timeout.min(PENDING_POLL);
            }
            if let Some(wakeup) = self.controller.next_wakeup() {
                timeout = timeout.min(wakeup.saturating_duration_since(now));
            }

            match rx.recv_timeout(timeout) {
                Ok(Request::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Request::Refresh) => self.refresh(),
                Ok(Request::Command(command)) => self.command(command),
                Ok(Request::ToggleTarget(preferred)) => {
                    match self.controller.hotkey_target(preferred.as_ref()) {
                        Some(id) => self.command(Command::Toggle(id)),
                        None => log::info!("toggle requested but no device is paired"),
                    }
                }
                Ok(Request::Configure(config)) => self.controller.set_config(config),
                Err(RecvTimeoutError::Timeout) => {
                    let since = Instant::now() - self.last_refresh;
                    if self.controller.has_pending() || since >= poll {
                        self.refresh();
                    }
                }
            }

            let actions = self.controller.tick(Instant::now());
            self.execute(actions);
            self.publish();
        }
    }

    fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        match self.backend.list_devices() {
            Ok(devices) => self.controller.apply_snapshot(devices, Instant::now()),
            Err(err) => log::warn!("listing devices failed: {err}"),
        }
    }

    fn command(&mut self, command: Command) {
        match self.controller.handle_command(command, Instant::now()) {
            Ok(actions) => self.execute(actions),
            Err(err) => log::warn!("{err}"),
        }
    }

    fn execute(&mut self, actions: Vec<Action>) {
        for action in actions {
            let result = match &action {
                Action::Connect(id) => self.backend.connect(id),
                Action::Disconnect(id) => self.backend.disconnect(id),
            };
            if let Err(err) = result {
                log::warn!("{action:?} failed: {err}");
                self.controller
                    .action_failed(&action, err.to_string(), Instant::now());
            }
        }
    }

    fn publish(&mut self) {
        let views = self.controller.views();
        {
            let mut current = self.views.lock().unwrap_or_else(PoisonError::into_inner);
            if *current == views {
                return;
            }
            current.clone_from(&views);
        }
        (self.on_update)(&views);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;

    use super::*;
    use crate::device::{AudioDevice, ConnectionState};
    use crate::fake::{FakeBackend, FakeBehavior};

    const WAIT: Duration = Duration::from_secs(5);

    fn spawn(
        backend: &FakeBackend,
        config: ControllerConfig,
    ) -> (Runtime, Receiver<Vec<DeviceView>>) {
        let (tx, rx) = mpsc::channel();
        let runtime = Runtime::spawn(
            Arc::new(backend.clone()),
            config,
            Box::new(move |views| {
                let _ = tx.send(views.to_vec());
            }),
        );
        (runtime, rx)
    }

    /// Waits until a published update satisfies `predicate`.
    fn wait_for(
        rx: &Receiver<Vec<DeviceView>>,
        predicate: impl Fn(&[DeviceView]) -> bool,
    ) -> Vec<DeviceView> {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let views = rx.recv_timeout(left).expect("timed out waiting for update");
            if predicate(&views) {
                return views;
            }
        }
    }

    fn state_is(views: &[DeviceView], state: ConnectionState) -> bool {
        views.first().is_some_and(|view| view.state == state)
    }

    fn buds() -> DeviceId {
        DeviceId::new("buds")
    }

    fn backend(connected: bool) -> FakeBackend {
        FakeBackend::new(vec![AudioDevice::new("buds", "Buds", connected)])
    }

    #[test]
    fn publishes_initial_devices() {
        let backend = backend(true);
        let (runtime, rx) = spawn(&backend, ControllerConfig::default());
        wait_for(&rx, |views| state_is(views, ConnectionState::Connected));
        assert_eq!(runtime.handle().views().len(), 1);
        assert_eq!(runtime.handle().backend_name(), "demo");
    }

    #[test]
    fn connect_and_disconnect_round_trip() {
        let backend = backend(false).with_latency(Duration::from_millis(50));
        let (runtime, rx) = spawn(&backend, ControllerConfig::default());
        let handle = runtime.handle();
        wait_for(&rx, |views| state_is(views, ConnectionState::Disconnected));

        handle.send(Command::Connect(buds()));
        wait_for(&rx, |views| state_is(views, ConnectionState::Connected));

        handle.send(Command::Disconnect(buds()));
        wait_for(&rx, |views| state_is(views, ConnectionState::Disconnected));
        assert_eq!(
            backend.calls(),
            vec![Action::Connect(buds()), Action::Disconnect(buds())]
        );
    }

    #[test]
    fn toggle_target_uses_hotkey_resolution() {
        let backend = backend(false);
        let (runtime, rx) = spawn(&backend, ControllerConfig::default());
        wait_for(&rx, |views| !views.is_empty());
        runtime.handle().toggle_target(None);
        wait_for(&rx, |views| state_is(views, ConnectionState::Connected));
    }

    #[test]
    fn rejected_request_surfaces_error() {
        let backend = backend(false);
        backend.set_behavior(FakeBehavior::Reject);
        let (runtime, rx) = spawn(&backend, ControllerConfig::default());
        wait_for(&rx, |views| !views.is_empty());

        runtime.handle().send(Command::Connect(buds()));
        let views = wait_for(&rx, |views| {
            views.first().is_some_and(|view| view.last_error.is_some())
        });
        assert_eq!(views[0].state, ConnectionState::Disconnected);
    }

    #[test]
    fn external_drop_triggers_auto_reconnect() {
        let backend = backend(true);
        let config = ControllerConfig {
            reconnect: crate::ReconnectPolicy {
                initial_delay_ms: 50,
                max_delay_ms: 50,
                max_attempts: 3,
            },
            ..ControllerConfig::default()
        };
        let (_runtime, rx) = spawn(&backend, config);
        wait_for(&rx, |views| state_is(views, ConnectionState::Connected));

        backend.set_connected(&buds(), false);
        wait_for(&rx, |views| state_is(views, ConnectionState::Disconnected));
        wait_for(&rx, |views| state_is(views, ConnectionState::Connected));
        assert_eq!(backend.calls(), vec![Action::Connect(buds())]);
    }

    #[test]
    fn dropping_runtime_stops_worker_and_unsubscribes() {
        let backend = backend(false);
        let (runtime, rx) = spawn(&backend, ControllerConfig::default());
        wait_for(&rx, |views| !views.is_empty());
        assert!(backend.is_watched());
        drop(runtime);
        assert!(!backend.is_watched());
    }
}
