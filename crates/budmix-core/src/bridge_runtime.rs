//! Runs a [`BridgeController`] against a [`SourceBackend`] on its own worker
//! thread. Opening a stream can block for several seconds, so it never holds
//! up the buds' connect/disconnect runtime.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::backend::ChangeNotifier;
use crate::bridge::{BridgeAction, BridgeController, SourceBackend, SourceView};
use crate::device::DeviceId;
use crate::reconnect::ReconnectPolicy;

/// Safety-net refresh in case a change notification is missed.
const IDLE_POLL: Duration = Duration::from_secs(30);
/// Refresh interval when the backend cannot send notifications.
const FALLBACK_POLL: Duration = Duration::from_secs(5);

enum Request {
    SetEnabled(DeviceId, bool),
    SetSuspended(bool),
    Refresh,
    Shutdown,
}

/// Called with the full list of phones every time it changes.
pub type SourceListener = Box<dyn Fn(&[SourceView]) + Send>;

#[derive(Clone)]
pub struct BridgeHandle {
    tx: Sender<Request>,
    views: Arc<Mutex<Vec<SourceView>>>,
}

impl BridgeHandle {
    pub fn set_enabled(&self, id: DeviceId, enabled: bool) {
        self.post(Request::SetEnabled(id, enabled));
    }

    /// Holds every phone stream off, or lets them come back (see
    /// [`crate::bridge::BridgeState::Paused`]).
    pub fn set_suspended(&self, suspended: bool) {
        self.post(Request::SetSuspended(suspended));
    }

    pub fn refresh(&self) {
        self.post(Request::Refresh);
    }

    pub fn views(&self) -> Vec<SourceView> {
        self.views
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn post(&self, request: Request) {
        if self.tx.send(request).is_err() {
            log::warn!("bridge runtime has stopped; request dropped");
        }
    }
}

/// Owns the bridge worker; dropping it closes every stream and stops it.
pub struct BridgeRuntime {
    handle: BridgeHandle,
    worker: Option<JoinHandle<()>>,
}

impl BridgeRuntime {
    pub fn spawn(
        backend: Arc<dyn SourceBackend>,
        enabled: Vec<DeviceId>,
        policy: ReconnectPolicy,
        on_update: SourceListener,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let views = Arc::new(Mutex::new(Vec::new()));
        let handle = BridgeHandle {
            tx: tx.clone(),
            views: Arc::clone(&views),
        };
        let worker = thread::Builder::new()
            .name("budmix-bridge".into())
            .spawn(move || {
                Worker {
                    backend,
                    controller: BridgeController::new(enabled, policy),
                    views,
                    on_update,
                    last_refresh: Instant::now(),
                }
                .run(tx, rx)
            })
            .expect("failed to spawn the bridge runtime thread");
        Self {
            handle,
            worker: Some(worker),
        }
    }

    pub fn handle(&self) -> BridgeHandle {
        self.handle.clone()
    }
}

impl Drop for BridgeRuntime {
    fn drop(&mut self) {
        let _ = self.handle.tx.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Worker {
    backend: Arc<dyn SourceBackend>,
    controller: BridgeController,
    views: Arc<Mutex<Vec<SourceView>>>,
    on_update: SourceListener,
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
                log::warn!("source notifications unavailable ({err}); polling instead");
                (None, FALLBACK_POLL)
            }
        };

        self.refresh();
        self.step();

        loop {
            let now = Instant::now();
            let mut timeout = poll.saturating_sub(now - self.last_refresh);
            if let Some(wakeup) = self.controller.next_wakeup() {
                timeout = timeout.min(wakeup.saturating_duration_since(now));
            }
            match rx.recv_timeout(timeout) {
                Ok(Request::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Request::Refresh) => self.refresh(),
                Ok(Request::SetSuspended(suspended)) => {
                    let actions = self.controller.set_suspended(suspended, Instant::now());
                    self.execute(actions);
                    self.refresh();
                }
                Ok(Request::SetEnabled(id, enabled)) => {
                    // Close first, so "Off" is only shown once the stream is gone.
                    // Opening happens in `step`, which shows "Connecting…" first.
                    let actions = self.controller.set_enabled(&id, enabled, Instant::now());
                    self.execute(actions);
                    self.refresh();
                }
                Err(RecvTimeoutError::Timeout) => {
                    if Instant::now() - self.last_refresh >= poll {
                        self.refresh();
                    }
                }
            }
            self.step();
        }

        // Leave nothing streaming behind.
        for view in self.controller.views() {
            if view.state != crate::bridge::BridgeState::Off
                && view.state != crate::bridge::BridgeState::Paused
            {
                let _ = self.backend.close(&view.id);
            }
        }
    }

    /// Fires due attempts, then publishes.
    fn step(&mut self) {
        let actions = self.controller.tick(Instant::now());
        // Show "Connecting…" before a slow open starts.
        self.publish();
        self.execute(actions);
        self.publish();
    }

    fn refresh(&mut self) {
        self.last_refresh = Instant::now();
        match self.backend.list_sources() {
            Ok(sources) => self.controller.apply_snapshot(sources, Instant::now()),
            Err(err) => log::warn!("listing audio sources failed: {err}"),
        }
    }

    fn execute(&mut self, actions: Vec<BridgeAction>) {
        for action in actions {
            match action {
                BridgeAction::Open(id) => {
                    let result = self.backend.open(&id).map_err(|err| err.to_string());
                    if let Err(err) = &result {
                        log::warn!("opening phone audio from {id} failed: {err}");
                    }
                    self.controller.open_finished(&id, result, Instant::now());
                }
                BridgeAction::Close(id) => {
                    if let Err(err) = self.backend.close(&id) {
                        log::warn!("closing phone audio from {id} failed: {err}");
                    }
                }
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
    use super::*;
    use crate::bridge::{AudioSource, BridgeState, default_retry_policy};
    use crate::fake::{FakeBehavior, FakeSourceBackend};

    const WAIT: Duration = Duration::from_secs(5);

    fn spawn(
        backend: &FakeSourceBackend,
        enabled: Vec<DeviceId>,
        policy: ReconnectPolicy,
    ) -> (BridgeRuntime, Receiver<Vec<SourceView>>) {
        let (tx, rx) = mpsc::channel();
        let runtime = BridgeRuntime::spawn(
            Arc::new(backend.clone()),
            enabled,
            policy,
            Box::new(move |views| {
                let _ = tx.send(views.to_vec());
            }),
        );
        (runtime, rx)
    }

    fn wait_for(
        rx: &Receiver<Vec<SourceView>>,
        predicate: impl Fn(&[SourceView]) -> bool,
    ) -> Vec<SourceView> {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let views = rx.recv_timeout(left).expect("timed out waiting for update");
            if predicate(&views) {
                return views;
            }
        }
    }

    fn state_is(views: &[SourceView], state: BridgeState) -> bool {
        views.first().is_some_and(|view| view.state == state)
    }

    fn phone_id() -> DeviceId {
        DeviceId::new("phone")
    }

    fn backend() -> FakeSourceBackend {
        FakeSourceBackend::new(vec![AudioSource::new("phone", "Pixel", false)])
    }

    #[test]
    fn enabling_streams_and_disabling_closes() {
        let backend = backend();
        let (runtime, rx) = spawn(&backend, Vec::new(), default_retry_policy());
        wait_for(&rx, |views| state_is(views, BridgeState::Off));

        runtime.handle().set_enabled(phone_id(), true);
        wait_for(&rx, |views| state_is(views, BridgeState::Streaming));
        assert!(backend.is_streaming(&phone_id()));

        runtime.handle().set_enabled(phone_id(), false);
        wait_for(&rx, |views| state_is(views, BridgeState::Off));
        assert!(!backend.is_streaming(&phone_id()));
    }

    #[test]
    fn remembered_phone_reconnects_after_failures() {
        let backend = backend();
        backend.set_behavior(FakeBehavior::Reject);
        let policy = ReconnectPolicy {
            initial_delay_ms: 20,
            max_delay_ms: 20,
            max_attempts: u32::MAX,
        };
        let (_runtime, rx) = spawn(&backend, vec![phone_id()], policy);
        wait_for(&rx, |views| {
            views.first().is_some_and(|view| view.last_error.is_some())
        });

        backend.set_behavior(FakeBehavior::Succeed);
        wait_for(&rx, |views| state_is(views, BridgeState::Streaming));
    }

    #[test]
    fn shutdown_closes_open_streams() {
        let backend = backend();
        let (runtime, rx) = spawn(&backend, vec![phone_id()], default_retry_policy());
        wait_for(&rx, |views| state_is(views, BridgeState::Streaming));
        drop(runtime);
        assert!(!backend.is_streaming(&phone_id()));
        assert!(!backend.is_watched());
    }
}
