//! In-memory backend for tests and the desktop app's demo mode.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use crate::backend::{BackendError, ChangeNotifier, DeviceBackend, Watch};
use crate::bridge::{AudioSource, SourceBackend, UnavailableSource};
use crate::controller::Action;
use crate::device::{AudioDevice, AudioProfile, DeviceId};

/// How the fake responds to connect/disconnect requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeBehavior {
    /// Apply the change after the configured latency.
    Succeed,
    /// Return an error immediately.
    Reject,
    /// Accept the request but never change anything (like busy buds).
    Ignore,
}

#[derive(Default)]
struct Inner {
    devices: Mutex<Vec<AudioDevice>>,
    notifier: Mutex<Option<ChangeNotifier>>,
    behavior: Mutex<Option<FakeBehavior>>,
    calls: Mutex<Vec<Action>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Inner {
    fn set_connected(&self, id: &DeviceId, connected: bool) -> bool {
        let changed = {
            let mut devices = lock(&self.devices);
            match devices.iter_mut().find(|device| &device.id == id) {
                Some(device) if device.connected != connected => {
                    device.connected = connected;
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.notify();
        }
        changed
    }

    fn notify(&self) {
        let notifier = lock(&self.notifier).clone();
        if let Some(notifier) = notifier {
            notifier();
        }
    }
}

/// A [`DeviceBackend`] whose devices live in memory.
#[derive(Clone)]
pub struct FakeBackend {
    inner: Arc<Inner>,
    latency: Duration,
}

impl FakeBackend {
    pub fn new(devices: Vec<AudioDevice>) -> Self {
        let inner = Inner {
            devices: Mutex::new(devices),
            ..Inner::default()
        };
        Self {
            inner: Arc::new(inner),
            latency: Duration::ZERO,
        }
    }

    /// Sample devices for demo mode. Names are clearly fictional.
    pub fn demo() -> Self {
        Self::new(vec![
            AudioDevice::new("demo-buds", "Demo Buds Pro", false)
                .with_profiles([AudioProfile::Stereo, AudioProfile::HandsFree]),
            AudioDevice::new("demo-headphones", "Demo Headphones", true)
                .with_profiles([AudioProfile::Stereo, AudioProfile::HandsFree]),
            AudioDevice::new("demo-speaker", "Demo Speaker", false),
        ])
        .with_latency(Duration::from_millis(1_500))
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    pub fn set_behavior(&self, behavior: FakeBehavior) {
        *lock(&self.inner.behavior) = Some(behavior);
    }

    /// Simulates a change made outside BudsLink (buds out of range, etc.).
    pub fn set_connected(&self, id: &DeviceId, connected: bool) -> bool {
        self.inner.set_connected(id, connected)
    }

    pub fn set_devices(&self, devices: Vec<AudioDevice>) {
        *lock(&self.inner.devices) = devices;
        self.inner.notify();
    }

    /// Every connect/disconnect request received so far.
    pub fn calls(&self) -> Vec<Action> {
        lock(&self.inner.calls).clone()
    }

    pub fn is_watched(&self) -> bool {
        lock(&self.inner.notifier).is_some()
    }

    fn request(&self, action: Action) -> Result<(), BackendError> {
        lock(&self.inner.calls).push(action.clone());
        let id = action.device_id().clone();
        if !lock(&self.inner.devices)
            .iter()
            .any(|device| device.id == id)
        {
            return Err(BackendError::NotFound(id));
        }
        let behavior = lock(&self.inner.behavior).unwrap_or(FakeBehavior::Succeed);
        let connected = matches!(action, Action::Connect(_));
        match behavior {
            FakeBehavior::Reject => Err(BackendError::Other("rejected by fake backend".into())),
            FakeBehavior::Ignore => Ok(()),
            FakeBehavior::Succeed if self.latency.is_zero() => {
                self.inner.set_connected(&id, connected);
                Ok(())
            }
            FakeBehavior::Succeed => {
                let inner = Arc::clone(&self.inner);
                let latency = self.latency;
                thread::spawn(move || {
                    thread::sleep(latency);
                    inner.set_connected(&id, connected);
                });
                Ok(())
            }
        }
    }
}

struct WatchGuard(Arc<Inner>);

impl Drop for WatchGuard {
    fn drop(&mut self) {
        *lock(&self.0.notifier) = None;
    }
}

impl DeviceBackend for FakeBackend {
    fn name(&self) -> &'static str {
        "demo"
    }

    fn list_devices(&self) -> Result<Vec<AudioDevice>, BackendError> {
        Ok(lock(&self.inner.devices).clone())
    }

    fn connect(&self, id: &DeviceId) -> Result<(), BackendError> {
        self.request(Action::Connect(id.clone()))
    }

    fn disconnect(&self, id: &DeviceId) -> Result<(), BackendError> {
        self.request(Action::Disconnect(id.clone()))
    }

    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError> {
        *lock(&self.inner.notifier) = Some(notify);
        Ok(Watch::new(WatchGuard(Arc::clone(&self.inner))))
    }
}

/// In-memory [`SourceBackend`]: phones that can stream to the laptop.
#[derive(Clone)]
pub struct FakeSourceBackend {
    sources: Arc<Mutex<Vec<AudioSource>>>,
    unavailable: Vec<UnavailableSource>,
    notifier: Arc<Mutex<Option<ChangeNotifier>>>,
    behavior: Arc<Mutex<FakeBehavior>>,
    latency: Duration,
}

impl FakeSourceBackend {
    pub fn new(sources: Vec<AudioSource>) -> Self {
        Self {
            sources: Arc::new(Mutex::new(sources)),
            unavailable: Vec::new(),
            notifier: Arc::new(Mutex::new(None)),
            behavior: Arc::new(Mutex::new(FakeBehavior::Succeed)),
            latency: Duration::ZERO,
        }
    }

    /// A clearly fictional phone for demo mode.
    pub fn demo() -> Self {
        Self::new(vec![AudioSource::new("demo-phone", "Demo Phone", false)])
            .with_latency(Duration::from_millis(1_500))
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// Paired phones that can't stream, as [`SourceBackend::unavailable`] reports them.
    pub fn with_unavailable(mut self, unavailable: Vec<UnavailableSource>) -> Self {
        self.unavailable = unavailable;
        self
    }

    pub fn set_behavior(&self, behavior: FakeBehavior) {
        *lock(&self.behavior) = behavior;
    }

    pub fn is_streaming(&self, id: &DeviceId) -> bool {
        lock(&self.sources)
            .iter()
            .any(|source| &source.id == id && source.streaming)
    }

    pub fn is_watched(&self) -> bool {
        lock(&self.notifier).is_some()
    }

    fn set_streaming(&self, id: &DeviceId, streaming: bool) -> Result<(), BackendError> {
        {
            let mut sources = lock(&self.sources);
            let source = sources
                .iter_mut()
                .find(|source| &source.id == id)
                .ok_or_else(|| BackendError::NotFound(id.clone()))?;
            source.streaming = streaming;
        }
        let notifier = lock(&self.notifier).clone();
        if let Some(notifier) = notifier {
            notifier();
        }
        Ok(())
    }
}

struct SourceWatchGuard(Arc<Mutex<Option<ChangeNotifier>>>);

impl Drop for SourceWatchGuard {
    fn drop(&mut self) {
        *lock(&self.0) = None;
    }
}

impl SourceBackend for FakeSourceBackend {
    fn name(&self) -> &'static str {
        "demo"
    }

    fn list_sources(&self) -> Result<Vec<AudioSource>, BackendError> {
        Ok(lock(&self.sources).clone())
    }

    fn unavailable(&self) -> Vec<UnavailableSource> {
        self.unavailable.clone()
    }

    fn diagnostics(&self) -> String {
        "Demo backend: simulated phones, nothing read from the system.".to_owned()
    }

    fn open(&self, id: &DeviceId) -> Result<(), BackendError> {
        match *lock(&self.behavior) {
            FakeBehavior::Reject => {
                return Err(BackendError::Other("the phone didn't answer".into()));
            }
            FakeBehavior::Ignore => return Ok(()),
            FakeBehavior::Succeed => {}
        }
        // Opening blocks, like the real Bluetooth handshake.
        thread::sleep(self.latency);
        self.set_streaming(id, true)
    }

    fn close(&self, id: &DeviceId) -> Result<(), BackendError> {
        self.set_streaming(id, false)
    }

    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError> {
        *lock(&self.notifier) = Some(notify);
        Ok(Watch::new(SourceWatchGuard(Arc::clone(&self.notifier))))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn buds() -> DeviceId {
        DeviceId::new("buds")
    }

    fn backend() -> FakeBackend {
        FakeBackend::new(vec![AudioDevice::new("buds", "Buds", false)])
    }

    #[test]
    fn connect_applies_and_notifies() {
        let backend = backend();
        let count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&count);
        let _watch = backend
            .watch(Arc::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            }))
            .unwrap();

        backend.connect(&buds()).unwrap();
        assert!(backend.list_devices().unwrap()[0].connected);
        assert_eq!(count.load(Ordering::SeqCst), 1);

        // No change, no notification.
        backend.connect(&buds()).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(backend.calls().len(), 2);
    }

    #[test]
    fn behaviors() {
        let backend = backend();
        backend.set_behavior(FakeBehavior::Reject);
        assert!(backend.connect(&buds()).is_err());

        backend.set_behavior(FakeBehavior::Ignore);
        backend.connect(&buds()).unwrap();
        assert!(!backend.list_devices().unwrap()[0].connected);

        assert_eq!(
            backend.connect(&DeviceId::new("ghost")),
            Err(BackendError::NotFound(DeviceId::new("ghost")))
        );
    }

    #[test]
    fn dropping_the_watch_unsubscribes() {
        let backend = backend();
        let watch = backend.watch(Arc::new(|| {})).unwrap();
        assert!(backend.is_watched());
        drop(watch);
        assert!(!backend.is_watched());
    }
}
