//! Runs the [`OutputRouter`] and the call-mode check against an
//! [`AudioBackend`] on its own worker thread, and plays the test tone.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::backend::ChangeNotifier;
use crate::device::DeviceId;
use crate::routing::{AudioBackend, Endpoint, OutputRouter, RouteAction};

/// While buds are connected: how often to look for an app using their mic.
/// Core Audio sends no notification for that.
const BUDS_POLL: Duration = Duration::from_secs(3);
/// Safety-net refresh otherwise.
const IDLE_POLL: Duration = Duration::from_secs(30);

/// What the UI shows about laptop audio.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioView {
    /// "Play laptop audio in my buds" is on.
    pub routing: bool,
    /// The buds BudsLink routes to.
    pub buds: Option<DeviceId>,
    /// Their playback endpoint, when connected.
    pub buds_output: Option<Endpoint>,
    pub default_output: Option<Endpoint>,
    /// The buds are the default output, so laptop audio plays in them.
    pub laptop_in_buds: bool,
    /// An app uses the buds' mic: they are in low-quality call mode (HFP).
    pub call_mode: bool,
    /// The output to give back later (persisted by the app).
    pub saved_output: Option<String>,
    pub last_error: Option<String>,
}

/// Called with the new view every time it changes.
pub type AudioListener = Box<dyn Fn(&AudioView) + Send>;

enum Request {
    Refresh,
    SetBuds(Option<DeviceId>),
    SetRouting(bool),
    PlayTone(Sender<Result<(), String>>),
    /// Restore the previous default output and stop; the sender, if any, is
    /// told when that is done.
    Shutdown(Option<Sender<()>>),
}

#[derive(Clone)]
pub struct AudioHandle {
    tx: Sender<Request>,
    view: Arc<Mutex<AudioView>>,
}

impl AudioHandle {
    /// The buds to route to: the connected ones the user cares about.
    pub fn set_buds(&self, buds: Option<DeviceId>) {
        self.post(Request::SetBuds(buds));
    }

    pub fn set_routing(&self, enabled: bool) {
        self.post(Request::SetRouting(enabled));
    }

    pub fn refresh(&self) {
        self.post(Request::Refresh);
    }

    /// Plays the test tone into the buds (or the default output) and blocks
    /// until it has finished.
    pub fn play_tone(&self) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        self.post(Request::PlayTone(tx));
        rx.recv()
            .unwrap_or_else(|_| Err("BudsLink's audio worker has stopped".to_owned()))
    }

    /// Gives the previous default output back and stops the worker. Waits up
    /// to two seconds, so quitting the app never hangs on it.
    pub fn stop(&self) {
        let (tx, rx) = mpsc::channel();
        self.post(Request::Shutdown(Some(tx)));
        let _ = rx.recv_timeout(Duration::from_secs(2));
    }

    pub fn view(&self) -> AudioView {
        self.view
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn post(&self, request: Request) {
        if self.tx.send(request).is_err() {
            log::warn!("audio runtime has stopped; request dropped");
        }
    }
}

/// Owns the audio worker; dropping it restores the previous default output
/// and stops it.
pub struct AudioRuntime {
    handle: AudioHandle,
    worker: Option<JoinHandle<()>>,
}

impl AudioRuntime {
    pub fn spawn(
        backend: Arc<dyn AudioBackend>,
        routing: bool,
        saved_output: Option<String>,
        on_update: AudioListener,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        let view = Arc::new(Mutex::new(AudioView::default()));
        let handle = AudioHandle {
            tx: tx.clone(),
            view: Arc::clone(&view),
        };
        let worker = thread::Builder::new()
            .name("budslink-audio".into())
            .spawn(move || {
                Worker {
                    backend,
                    router: OutputRouter::new(routing, saved_output),
                    buds: None,
                    buds_output: None,
                    default_output: None,
                    call_mode: false,
                    last_error: None,
                    view,
                    on_update,
                    last_refresh: Instant::now(),
                }
                .run(tx, rx)
            })
            .expect("failed to spawn the audio runtime thread");
        Self {
            handle,
            worker: Some(worker),
        }
    }

    pub fn handle(&self) -> AudioHandle {
        self.handle.clone()
    }
}

impl Drop for AudioRuntime {
    fn drop(&mut self) {
        let _ = self.handle.tx.send(Request::Shutdown(None));
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Worker {
    backend: Arc<dyn AudioBackend>,
    router: OutputRouter,
    buds: Option<DeviceId>,
    buds_output: Option<Endpoint>,
    default_output: Option<Endpoint>,
    call_mode: bool,
    last_error: Option<String>,
    view: Arc<Mutex<AudioView>>,
    on_update: AudioListener,
    last_refresh: Instant,
}

impl Worker {
    fn run(mut self, tx: Sender<Request>, rx: Receiver<Request>) {
        let notify: ChangeNotifier = Arc::new(move || {
            let _ = tx.send(Request::Refresh);
        });
        let _watch = self
            .backend
            .watch(notify)
            .inspect_err(|err| log::warn!("audio notifications unavailable ({err}); polling"))
            .ok();

        self.refresh_and_route();
        let mut done = None;
        loop {
            let poll = if self.buds_output.is_some() {
                BUDS_POLL
            } else {
                IDLE_POLL
            };
            let timeout = poll.saturating_sub(self.last_refresh.elapsed());
            match rx.recv_timeout(timeout) {
                Ok(Request::Shutdown(reply)) => {
                    done = reply;
                    break;
                }
                Err(RecvTimeoutError::Disconnected) => break,
                Ok(Request::Refresh) | Err(RecvTimeoutError::Timeout) => {}
                Ok(Request::SetBuds(buds)) => self.buds = buds,
                Ok(Request::SetRouting(enabled)) => {
                    self.read_state();
                    let action = self.router.set_enabled(
                        enabled,
                        self.buds_id(),
                        self.default_id(),
                    );
                    self.execute(action);
                }
                Ok(Request::PlayTone(reply)) => {
                    let target = self.buds_output.as_ref().map(|output| output.id.clone());
                    let result = self
                        .backend
                        .play_tone(target.as_deref())
                        .map_err(|err| err.to_string());
                    let _ = reply.send(result);
                }
            }
            self.refresh_and_route();
        }

        self.read_state();
        let action = self.router.shutdown(self.buds_id(), self.default_id());
        self.execute(action);
        if let Some(done) = done {
            let _ = done.send(());
        }
    }

    fn refresh_and_route(&mut self) {
        self.read_state();
        let action = self.router.update(self.buds_id(), self.default_id());
        if action.is_some() {
            self.execute(action);
            self.read_state();
        }
        self.publish();
    }

    fn read_state(&mut self) {
        self.last_refresh = Instant::now();
        self.default_output = self
            .backend
            .default_output()
            .inspect_err(|err| log::warn!("reading the default output failed: {err}"))
            .unwrap_or(None);
        let Some(buds) = self.buds.clone() else {
            self.buds_output = None;
            self.call_mode = false;
            return;
        };
        self.buds_output = self
            .backend
            .buds_output(&buds)
            .inspect_err(|err| log::warn!("finding the buds' output failed: {err}"))
            .unwrap_or(None);
        self.call_mode = self.buds_output.is_some()
            && self
                .backend
                .call_mode(&buds)
                .inspect_err(|err| log::debug!("call-mode check failed: {err}"))
                .unwrap_or(false);
    }

    fn execute(&mut self, action: Option<RouteAction>) {
        let Some(action) = action else {
            return;
        };
        let (endpoint, what) = match &action {
            RouteAction::UseBuds(id) => (id, "make the buds the default output"),
            RouteAction::Restore(id) => (id, "restore the previous default output"),
        };
        match self.backend.set_default_output(endpoint) {
            Ok(()) => {
                log::info!("{action:?}");
                self.last_error = None;
            }
            Err(err) => {
                log::warn!("couldn't {what}: {err}");
                self.last_error = Some(format!("Couldn't {what}: {err}"));
            }
        }
    }

    fn buds_id(&self) -> Option<&str> {
        self.buds_output.as_ref().map(|output| output.id.as_str())
    }

    fn default_id(&self) -> Option<&str> {
        self.default_output.as_ref().map(|output| output.id.as_str())
    }

    fn publish(&self) {
        let view = AudioView {
            routing: self.router.enabled(),
            buds: self.buds.clone(),
            buds_output: self.buds_output.clone(),
            default_output: self.default_output.clone(),
            laptop_in_buds: self.buds_id().is_some() && self.buds_id() == self.default_id(),
            call_mode: self.call_mode,
            saved_output: self.router.saved().map(str::to_owned),
            last_error: self.last_error.clone(),
        };
        let mut current = self.view.lock().unwrap_or_else(PoisonError::into_inner);
        if *current != view {
            *current = view.clone();
            drop(current);
            (self.on_update)(&view);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeAudioBackend;

    fn wait_for(handle: &AudioHandle, check: impl Fn(&AudioView) -> bool) -> AudioView {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let view = handle.view();
            if check(&view) {
                return view;
            }
            assert!(Instant::now() < deadline, "timed out; last view: {view:?}");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn spawn(backend: &Arc<FakeAudioBackend>, routing: bool) -> AudioRuntime {
        AudioRuntime::spawn(
            Arc::clone(backend) as Arc<dyn AudioBackend>,
            routing,
            None,
            Box::new(|_| {}),
        )
    }

    #[test]
    fn routes_to_the_buds_and_restores_on_drop() {
        let backend = Arc::new(FakeAudioBackend::new());
        let runtime = spawn(&backend, true);
        let handle = runtime.handle();
        handle.set_buds(Some(DeviceId::new(FakeAudioBackend::BUDS)));
        let view = wait_for(&handle, |view| view.laptop_in_buds);
        assert_eq!(view.saved_output.as_deref(), Some(FakeAudioBackend::SPEAKERS_ENDPOINT));
        drop(runtime);
        assert_eq!(backend.default_id().as_deref(), Some(FakeAudioBackend::SPEAKERS_ENDPOINT));
    }

    #[test]
    fn stop_restores_before_returning() {
        let backend = Arc::new(FakeAudioBackend::new());
        let runtime = spawn(&backend, true);
        let handle = runtime.handle();
        handle.set_buds(Some(DeviceId::new(FakeAudioBackend::BUDS)));
        wait_for(&handle, |view| view.laptop_in_buds);
        handle.stop();
        assert_eq!(backend.default_id().as_deref(), Some(FakeAudioBackend::SPEAKERS_ENDPOINT));
        drop(runtime);
    }

    #[test]
    fn routing_off_leaves_the_default_alone() {
        let backend = Arc::new(FakeAudioBackend::new());
        let runtime = spawn(&backend, false);
        let handle = runtime.handle();
        handle.set_buds(Some(DeviceId::new(FakeAudioBackend::BUDS)));
        let view = wait_for(&handle, |view| view.buds_output.is_some());
        assert!(!view.laptop_in_buds);
        assert_eq!(backend.default_id().as_deref(), Some(FakeAudioBackend::SPEAKERS_ENDPOINT));
    }

    #[test]
    fn reports_call_mode() {
        let backend = Arc::new(FakeAudioBackend::new());
        let runtime = spawn(&backend, true);
        let handle = runtime.handle();
        handle.set_buds(Some(DeviceId::new(FakeAudioBackend::BUDS)));
        wait_for(&handle, |view| view.laptop_in_buds);
        backend.set_call_mode(true);
        handle.refresh();
        wait_for(&handle, |view| view.call_mode);
    }

    #[test]
    fn plays_the_tone_on_the_buds() {
        let backend = Arc::new(FakeAudioBackend::new());
        let runtime = spawn(&backend, true);
        let handle = runtime.handle();
        handle.set_buds(Some(DeviceId::new(FakeAudioBackend::BUDS)));
        wait_for(&handle, |view| view.buds_output.is_some());
        assert_eq!(handle.play_tone(), Ok(()));
        assert_eq!(
            backend.tones(),
            vec![Some(FakeAudioBackend::BUDS_ENDPOINT.to_owned())]
        );
    }
}
