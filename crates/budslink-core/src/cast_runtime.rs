//! Runs the [`CastController`] and the [`PhoneRouter`] against a
//! [`CastBackend`] on their own worker thread.
//!
//! It publishes a [`CastStatus`], including whether the Bluetooth phone link
//! must stay off, which the app hands to the bridge runtime.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::cast::{
    CastAction, CastController, CastPhase, CastView, StartOutcome, receiver_retry_policy,
};
use crate::phone_router::{PhoneMode, PhoneRouter};
use crate::volume::Level;

/// What the receiver reports after it started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastEvent {
    PhoneConnected {
        name: String,
        pin: Option<String>,
    },
    StreamStarted,
    PhoneDisconnected,
    PlaybackFailed(String),
    /// The receiver stopped listening by itself.
    Lost(String),
}

pub type CastEvents = Arc<dyn Fn(CastEvent) + Send + Sync>;

/// The Wi-Fi receiver side of the operating system.
pub trait CastBackend: Send + Sync {
    /// See [`crate::cast`]: Private (or Domain) networks only.
    fn network_private(&self) -> Option<bool>;
    /// Starts the receiver; `events` is called from other threads.
    fn start(&self, events: CastEvents) -> StartOutcome;
    fn stop(&self);
    /// Lets the current phone play.
    fn play(&self);
    /// Disconnects the current phone.
    fn reject(&self);
    /// The "Phone over Wi-Fi" slider.
    fn set_volume(&self, level: Level);
    /// Shows or hides the cast's picture window (the audio plays either way).
    fn show_video(&self, visible: bool);
}

/// What the UI shows about phone audio over Wi-Fi, plus the Bluetooth rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CastStatus {
    #[serde(flatten)]
    pub cast: CastView,
    pub mode: PhoneMode,
    /// The Bluetooth phone link must stay off (the phone casts, or Wi-Fi only).
    pub bluetooth_suspended: bool,
    pub video_visible: bool,
    pub last_error: Option<String>,
}

pub type CastListener = Box<dyn Fn(&CastStatus) + Send>;

/// How often to look at the network profile; Windows sends no cheap event.
const NETWORK_POLL: Duration = Duration::from_secs(10);

enum Request {
    Event(CastEvent),
    SetMode(PhoneMode),
    Approve(bool),
    Reject,
    Forget(String),
    SetVolume(Level),
    ShowVideo(bool),
    CheckNetwork,
    Shutdown(Option<Sender<()>>),
}

#[derive(Clone)]
pub struct CastHandle {
    tx: Sender<Request>,
    status: Arc<Mutex<Option<CastStatus>>>,
}

impl CastHandle {
    pub fn set_mode(&self, mode: PhoneMode) {
        self.post(Request::SetMode(mode));
    }

    /// Allows the waiting phone; `remember` puts it on the allowlist.
    pub fn approve(&self, remember: bool) {
        self.post(Request::Approve(remember));
    }

    pub fn reject(&self) {
        self.post(Request::Reject);
    }

    pub fn forget(&self, name: String) {
        self.post(Request::Forget(name));
    }

    pub fn set_volume(&self, level: Level) {
        self.post(Request::SetVolume(level));
    }

    pub fn show_video(&self, visible: bool) {
        self.post(Request::ShowVideo(visible));
    }

    /// Looks at the network now (e.g. after a Wi-Fi change or wake-up).
    pub fn check_network(&self) {
        self.post(Request::CheckNetwork);
    }

    /// Stops the receiver; waits up to two seconds.
    pub fn stop(&self) {
        let (tx, rx) = mpsc::channel();
        self.post(Request::Shutdown(Some(tx)));
        let _ = rx.recv_timeout(Duration::from_secs(2));
    }

    pub fn status(&self) -> Option<CastStatus> {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn post(&self, request: Request) {
        if self.tx.send(request).is_err() {
            log::warn!("cast runtime has stopped; request dropped");
        }
    }
}

/// Settings the cast runtime starts with.
#[derive(Debug, Clone)]
pub struct CastConfig {
    pub mode: PhoneMode,
    pub allowed: Vec<String>,
    pub volume: Level,
    pub show_video: bool,
}

pub struct CastRuntime {
    handle: CastHandle,
    worker: Option<JoinHandle<()>>,
}

impl CastRuntime {
    /// `on_allowed` is called with the new allowlist when it changes, so
    /// the app can save it.
    pub fn spawn(
        backend: Arc<dyn CastBackend>,
        config: CastConfig,
        on_update: CastListener,
        on_allowed: Box<dyn Fn(Vec<String>) + Send>,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(None));
        let handle = CastHandle {
            tx: tx.clone(),
            status: Arc::clone(&status),
        };
        let worker = thread::Builder::new()
            .name("budslink-cast".into())
            .spawn(move || {
                let now = Instant::now();
                let mut worker = Worker {
                    controller: CastController::new(
                        config.mode.uses_wifi(),
                        config.allowed,
                        receiver_retry_policy(),
                    ),
                    router: PhoneRouter::new(config.mode),
                    backend,
                    show_video: config.show_video,
                    last_error: None,
                    status,
                    on_update,
                    on_allowed,
                    last_network_check: now,
                    private: None,
                };
                worker.backend.set_volume(config.volume);
                worker.run(tx, rx);
            })
            .expect("failed to spawn the cast runtime thread");
        Self {
            handle,
            worker: Some(worker),
        }
    }

    pub fn handle(&self) -> CastHandle {
        self.handle.clone()
    }
}

impl Drop for CastRuntime {
    fn drop(&mut self) {
        let _ = self.handle.tx.send(Request::Shutdown(None));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Worker {
    controller: CastController,
    router: PhoneRouter,
    backend: Arc<dyn CastBackend>,
    show_video: bool,
    last_error: Option<String>,
    status: Arc<Mutex<Option<CastStatus>>>,
    on_update: CastListener,
    on_allowed: Box<dyn Fn(Vec<String>) + Send>,
    last_network_check: Instant,
    /// Last network answer, to log only changes.
    private: Option<bool>,
}

impl Worker {
    fn run(&mut self, tx: Sender<Request>, rx: Receiver<Request>) {
        let events: CastEvents = Arc::new(move |event| {
            let _ = tx.send(Request::Event(event));
        });
        self.check_network(&events);
        self.publish();
        let mut done = None;
        loop {
            let now = Instant::now();
            let mut timeout = NETWORK_POLL.saturating_sub(now - self.last_network_check);
            for wakeup in [self.controller.next_wakeup(), self.router.next_wakeup(now)]
                .into_iter()
                .flatten()
            {
                timeout = timeout.min(wakeup.saturating_duration_since(now));
            }
            match rx.recv_timeout(timeout) {
                Ok(Request::Shutdown(reply)) => {
                    done = reply;
                    break;
                }
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {
                    if self.last_network_check.elapsed() >= NETWORK_POLL {
                        self.check_network(&events);
                    }
                    let actions = self.controller.tick(Instant::now());
                    self.execute(actions, &events);
                }
                Ok(Request::CheckNetwork) => self.check_network(&events),
                Ok(Request::Event(event)) => self.on_event(event, &events),
                Ok(Request::SetMode(mode)) => {
                    self.router.set_mode(mode);
                    let actions = self
                        .controller
                        .set_enabled(mode.uses_wifi(), Instant::now());
                    self.execute(actions, &events);
                }
                Ok(Request::Approve(remember)) => {
                    let (actions, added) = self.controller.approve(remember);
                    self.execute(actions, &events);
                    if added.is_some() {
                        (self.on_allowed)(self.controller.allowed().to_vec());
                    }
                }
                Ok(Request::Reject) => {
                    let actions = self.controller.reject();
                    self.execute(actions, &events);
                }
                Ok(Request::Forget(name)) => {
                    if self.controller.forget(&name) {
                        (self.on_allowed)(self.controller.allowed().to_vec());
                    }
                }
                Ok(Request::SetVolume(level)) => self.backend.set_volume(level),
                Ok(Request::ShowVideo(visible)) => {
                    self.show_video = visible;
                    if self.controller.view().phase == CastPhase::Playing || !visible {
                        self.backend.show_video(visible);
                    }
                }
            }
            self.publish();
        }
        self.backend.stop();
        if let Some(done) = done {
            let _ = done.send(());
        }
    }

    fn check_network(&mut self, events: &CastEvents) {
        self.last_network_check = Instant::now();
        let private = self.backend.network_private().unwrap_or(false);
        if self.private != Some(private) {
            log::info!("trusted (Private) network: {private}");
            self.private = Some(private);
        }
        let actions = self.controller.set_private(private, Instant::now());
        self.execute(actions, events);
    }

    fn on_event(&mut self, event: CastEvent, events: &CastEvents) {
        let now = Instant::now();
        match event {
            CastEvent::PhoneConnected { name, pin } => {
                log::info!("phone casting: {name}");
                self.last_error = None;
                let actions = self.controller.phone_connected(name, pin);
                self.execute(actions, events);
            }
            CastEvent::StreamStarted => {
                self.controller.stream_started();
                if self.show_video && self.controller.view().phase == CastPhase::Playing {
                    self.backend.show_video(true);
                }
            }
            CastEvent::PhoneDisconnected => {
                self.controller.phone_disconnected();
                self.backend.show_video(false);
            }
            CastEvent::PlaybackFailed(message) => {
                log::warn!("cast playback failed: {message}");
                self.last_error = Some(format!("The phone's stream couldn't play: {message}"));
            }
            CastEvent::Lost(message) => {
                let actions = self.controller.receiver_lost(message, now);
                self.execute(actions, events);
            }
        }
        self.router.set_casting(self.controller.casting(), now);
    }

    fn execute(&mut self, actions: Vec<CastAction>, events: &CastEvents) {
        for action in actions {
            match action {
                CastAction::StartReceiver => {
                    let outcome = self.backend.start(Arc::clone(events));
                    log::info!("Miracast receiver start: {outcome:?}");
                    let more = self.controller.start_finished(outcome, Instant::now());
                    self.execute(more, events);
                }
                CastAction::StopReceiver => {
                    self.backend.show_video(false);
                    self.backend.stop();
                }
                CastAction::Play => {
                    self.backend.play();
                    // A stream that started before the approval plays now.
                    self.controller.stream_started();
                    if self.show_video {
                        self.backend.show_video(true);
                    }
                }
                CastAction::Reject => {
                    self.backend.show_video(false);
                    self.backend.reject();
                }
            }
        }
        self.router
            .set_casting(self.controller.casting(), Instant::now());
    }

    fn publish(&self) {
        let status = CastStatus {
            cast: self.controller.view(),
            mode: self.router.mode(),
            bluetooth_suspended: self.router.bluetooth_suspended(Instant::now()),
            video_visible: self.show_video,
            last_error: self.last_error.clone(),
        };
        let mut current = self.status.lock().unwrap_or_else(PoisonError::into_inner);
        if current.as_ref() != Some(&status) {
            *current = Some(status.clone());
            drop(current);
            (self.on_update)(&status);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cast::ReceiverState;
    use crate::fake::FakeCastBackend;

    fn spawn(backend: &Arc<FakeCastBackend>, mode: PhoneMode, allowed: &[&str]) -> CastRuntime {
        CastRuntime::spawn(
            Arc::clone(backend) as Arc<dyn CastBackend>,
            CastConfig {
                mode,
                allowed: allowed.iter().map(|name| (*name).to_owned()).collect(),
                volume: Level::FULL,
                show_video: true,
            },
            Box::new(|_| {}),
            Box::new(|_| {}),
        )
    }

    fn wait_for(handle: &CastHandle, check: impl Fn(&CastStatus) -> bool) -> CastStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = handle.status() {
                if check(&status) {
                    return status;
                }
            }
            assert!(
                Instant::now() < deadline,
                "timed out; last: {:?}",
                handle.status()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn starts_on_a_private_network_and_plays_an_allowed_phone() {
        let backend = Arc::new(FakeCastBackend::new(true));
        let runtime = spawn(&backend, PhoneMode::Auto, &["Infinix"]);
        let handle = runtime.handle();
        wait_for(&handle, |status| {
            status.cast.receiver == ReceiverState::Ready
        });

        backend.cast("Infinix");
        let status = wait_for(&handle, |status| status.cast.phase == CastPhase::Playing);
        assert!(status.bluetooth_suspended, "no echo: Bluetooth is held off");
        assert_eq!(backend.plays(), 1);
        assert!(backend.video_visible());

        backend.end_cast();
        let status = wait_for(&handle, |status| status.cast.phase == CastPhase::Idle);
        assert!(status.bluetooth_suspended, "still within the grace period");
        wait_for(&handle, |status| !status.bluetooth_suspended);
        assert!(!backend.video_visible());
    }

    #[test]
    fn stays_off_on_a_public_network() {
        let backend = Arc::new(FakeCastBackend::new(false));
        let runtime = spawn(&backend, PhoneMode::Auto, &[]);
        let handle = runtime.handle();
        let status = wait_for(&handle, |status| {
            status.cast.receiver == ReceiverState::NotPrivate
        });
        assert!(!status.bluetooth_suspended);
        assert!(!backend.started());
    }

    #[test]
    fn a_new_phone_waits_for_approval_and_is_remembered() {
        let backend = Arc::new(FakeCastBackend::new(true));
        let saved = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&saved);
        let runtime = CastRuntime::spawn(
            Arc::clone(&backend) as Arc<dyn CastBackend>,
            CastConfig {
                mode: PhoneMode::Auto,
                allowed: Vec::new(),
                volume: Level::FULL,
                show_video: false,
            },
            Box::new(|_| {}),
            Box::new(move |allowed| *sink.lock().unwrap() = allowed),
        );
        let handle = runtime.handle();
        wait_for(&handle, |status| {
            status.cast.receiver == ReceiverState::Ready
        });
        backend.cast("Guest");
        wait_for(&handle, |status| {
            status.cast.phase == CastPhase::NeedsApproval
        });
        assert_eq!(backend.plays(), 0);
        handle.approve(true);
        wait_for(&handle, |status| status.cast.phase == CastPhase::Playing);
        assert_eq!(backend.plays(), 1);
        assert_eq!(*saved.lock().unwrap(), vec!["Guest".to_owned()]);
        assert!(
            !backend.video_visible(),
            "video stays hidden when turned off"
        );
    }

    #[test]
    fn bluetooth_only_mode_never_starts_the_receiver() {
        let backend = Arc::new(FakeCastBackend::new(true));
        let runtime = spawn(&backend, PhoneMode::BluetoothOnly, &[]);
        let handle = runtime.handle();
        let status = wait_for(&handle, |status| status.cast.receiver == ReceiverState::Off);
        assert!(!status.bluetooth_suspended);
        handle.set_mode(PhoneMode::WifiOnly);
        let status = wait_for(&handle, |status| {
            status.cast.receiver == ReceiverState::Ready
        });
        assert!(status.bluetooth_suspended);
    }

    #[test]
    fn stop_stops_the_receiver() {
        let backend = Arc::new(FakeCastBackend::new(true));
        let runtime = spawn(&backend, PhoneMode::Auto, &[]);
        let handle = runtime.handle();
        wait_for(&handle, |status| {
            status.cast.receiver == ReceiverState::Ready
        });
        handle.stop();
        assert!(!backend.started());
        drop(runtime);
    }
}
