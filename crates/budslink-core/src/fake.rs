//! In-memory backend for tests and the desktop app's demo mode.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use crate::backend::{BackendError, ChangeNotifier, DeviceBackend, Watch};
use crate::bridge::{AudioSource, SourceBackend, UnavailableSource};
use crate::cast::StartOutcome;
use crate::cast_runtime::{CastBackend, CastEvent, CastEvents};
use crate::controller::Action;
use crate::device::{AudioDevice, AudioProfile, DeviceId};
use crate::routing::{AudioBackend, Endpoint};
use crate::volume::Level;

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

const TONE_PLAYBACK: Duration = Duration::from_millis(700);

/// An [`AudioBackend`] with a speaker output and buds that are connected
/// either always ([`FakeAudioBackend::new`]) or whenever a [`FakeBackend`]
/// says so ([`FakeAudioBackend::linked`], used by demo mode).
pub struct FakeAudioBackend {
    devices: Option<FakeBackend>,
    default: Mutex<Option<String>>,
    call_mode: Mutex<bool>,
    tones: Mutex<Vec<Option<String>>>,
    tone_delay: Duration,
    masters: Mutex<Vec<Level>>,
    sessions: Mutex<Option<(Level, Level)>>,
    notifier: Arc<Mutex<Option<ChangeNotifier>>>,
}

impl FakeAudioBackend {
    pub const BUDS: &str = "demo-buds";
    pub const BUDS_ENDPOINT: &str = "endpoint-demo-buds";
    pub const SPEAKERS_ENDPOINT: &str = "endpoint-speakers";

    /// Every buds id counts as connected; the speakers start as default.
    pub fn new() -> Self {
        Self {
            devices: None,
            default: Mutex::new(Some(Self::SPEAKERS_ENDPOINT.to_owned())),
            call_mode: Mutex::new(false),
            tones: Mutex::new(Vec::new()),
            tone_delay: Duration::ZERO,
            masters: Mutex::new(Vec::new()),
            sessions: Mutex::new(None),
            notifier: Arc::default(),
        }
    }

    /// Buds are connected when `devices` lists them as connected.
    pub fn linked(devices: FakeBackend) -> Self {
        Self {
            devices: Some(devices),
            tone_delay: TONE_PLAYBACK,
            ..Self::new()
        }
    }

    pub fn default_id(&self) -> Option<String> {
        lock(&self.default).clone()
    }

    /// Simulates the user picking another output in Windows.
    pub fn set_default(&self, endpoint: Option<&str>) {
        *lock(&self.default) = endpoint.map(str::to_owned);
        let notifier = lock(&self.notifier).clone();
        if let Some(notifier) = notifier {
            notifier();
        }
    }

    pub fn set_call_mode(&self, active: bool) {
        *lock(&self.call_mode) = active;
    }

    /// Every master level set on the buds, in order.
    pub fn masters(&self) -> Vec<Level> {
        lock(&self.masters).clone()
    }

    /// The last (laptop, own) session levels applied.
    pub fn session_levels(&self) -> Option<(Level, Level)> {
        *lock(&self.sessions)
    }

    /// The endpoints the test tone was played on, in order.
    pub fn tones(&self) -> Vec<Option<String>> {
        lock(&self.tones).clone()
    }
}

impl Default for FakeAudioBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioBackend for FakeAudioBackend {
    fn default_output(&self) -> Result<Option<Endpoint>, BackendError> {
        Ok(self.default_id().map(|id| {
            let name = match id.strip_prefix("endpoint-") {
                Some("speakers") => "Demo Laptop Speakers".to_owned(),
                Some(buds) => format!("Headphones ({buds})"),
                None => id.clone(),
            };
            Endpoint::new(id, name)
        }))
    }

    fn buds_output(&self, buds: &DeviceId) -> Result<Option<Endpoint>, BackendError> {
        let name = match &self.devices {
            None => format!("Headphones ({buds})"),
            Some(devices) => {
                let connected = devices
                    .list_devices()?
                    .into_iter()
                    .find(|device| &device.id == buds && device.connected);
                match connected {
                    Some(device) => format!("Headphones ({})", device.name),
                    None => return Ok(None),
                }
            }
        };
        Ok(Some(Endpoint::new(format!("endpoint-{buds}"), name)))
    }

    fn set_default_output(&self, endpoint_id: &str) -> Result<(), BackendError> {
        self.set_default(Some(endpoint_id));
        Ok(())
    }

    fn call_mode(&self, _buds: &DeviceId) -> Result<bool, BackendError> {
        Ok(*lock(&self.call_mode))
    }

    fn set_master_volume(&self, _endpoint_id: &str, level: Level) -> Result<(), BackendError> {
        lock(&self.masters).push(level);
        Ok(())
    }

    fn apply_session_volumes(
        &self,
        _endpoint_id: &str,
        laptop: Level,
        own: Level,
    ) -> Result<(), BackendError> {
        *lock(&self.sessions) = Some((laptop, own));
        Ok(())
    }

    fn play_tone(&self, endpoint_id: Option<&str>) -> Result<(), BackendError> {
        lock(&self.tones).push(endpoint_id.map(str::to_owned));
        thread::sleep(self.tone_delay);
        Ok(())
    }

    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError> {
        *lock(&self.notifier) = Some(notify);
        Ok(Watch::new(SourceWatchGuard(Arc::clone(&self.notifier))))
    }
}

/// A [`CastBackend`] that pretends to be a Miracast receiver. Tests make a
/// phone cast with [`FakeCastBackend::cast`]; demo mode casts "Demo Phone"
/// a few seconds after the receiver starts.
pub struct FakeCastBackend {
    private: bool,
    demo_cast_after: Option<Duration>,
    events: Mutex<Option<CastEvents>>,
    started: AtomicBool,
    plays: AtomicUsize,
    video: AtomicBool,
}

impl FakeCastBackend {
    pub fn new(private: bool) -> Self {
        Self {
            private,
            demo_cast_after: None,
            events: Mutex::new(None),
            started: AtomicBool::new(false),
            plays: AtomicUsize::new(0),
            video: AtomicBool::new(false),
        }
    }

    /// For demo mode: a fictional phone starts casting 6 s after start.
    pub fn demo() -> Self {
        Self {
            demo_cast_after: Some(Duration::from_secs(6)),
            ..Self::new(true)
        }
    }

    /// Simulates a phone that starts casting.
    pub fn cast(&self, name: &str) {
        let events = lock(&self.events).clone();
        if let Some(events) = events {
            events(CastEvent::PhoneConnected {
                name: name.to_owned(),
                pin: None,
            });
            events(CastEvent::StreamStarted);
        }
    }

    /// Simulates the phone stopping its cast.
    pub fn end_cast(&self) {
        let events = lock(&self.events).clone();
        if let Some(events) = events {
            events(CastEvent::PhoneDisconnected);
        }
    }

    pub fn started(&self) -> bool {
        self.started.load(Ordering::SeqCst)
    }

    pub fn plays(&self) -> usize {
        self.plays.load(Ordering::SeqCst)
    }

    pub fn video_visible(&self) -> bool {
        self.video.load(Ordering::SeqCst)
    }
}

impl CastBackend for FakeCastBackend {
    fn network_private(&self) -> Option<bool> {
        Some(self.private)
    }

    fn start(&self, events: CastEvents) -> StartOutcome {
        *lock(&self.events) = Some(Arc::clone(&events));
        self.started.store(true, Ordering::SeqCst);
        if let Some(delay) = self.demo_cast_after {
            thread::spawn(move || {
                thread::sleep(delay);
                events(CastEvent::PhoneConnected {
                    name: "Demo Phone".to_owned(),
                    pin: None,
                });
                events(CastEvent::StreamStarted);
            });
        }
        StartOutcome::Ready
    }

    fn stop(&self) {
        *lock(&self.events) = None;
        self.started.store(false, Ordering::SeqCst);
    }

    fn play(&self) {
        self.plays.fetch_add(1, Ordering::SeqCst);
    }

    fn reject(&self) {}

    fn set_volume(&self, _level: Level) {}

    fn show_video(&self, visible: bool) {
        self.video.store(visible, Ordering::SeqCst);
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
