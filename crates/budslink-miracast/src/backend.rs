//! [`CastBackend`] for Windows: the built-in Miracast receiver, the network
//! check and the video window.

use std::sync::{Mutex, MutexGuard, PoisonError};

use budslink_core::{CastBackend, CastEvent, CastEvents, Level, StartOutcome};

use crate::probe::{Listening, Verdict};
use crate::video::VideoWindow;
use crate::{Receiver, ReceiverEvent, ReceiverOptions, network};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Default)]
pub struct MiracastBackend {
    receiver: Mutex<Option<Receiver>>,
    /// Created the first time a picture is shown.
    video: Mutex<Option<VideoWindow>>,
    volume: Mutex<Option<Level>>,
}

impl MiracastBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn apply_volume(&self, receiver: &Receiver) {
        if let Some(level) = *lock(&self.volume) {
            receiver.set_volume(f64::from(level.percent.min(100)) / 100.0, level.muted);
        }
    }
}

impl CastBackend for MiracastBackend {
    fn network_private(&self) -> Option<bool> {
        network::connected_networks_private()
    }

    fn start(&self, events: CastEvents) -> StartOutcome {
        let listener = move |event: ReceiverEvent| {
            let event = match event {
                ReceiverEvent::PhoneConnected { name, pin } => {
                    CastEvent::PhoneConnected { name, pin }
                }
                ReceiverEvent::StreamStarted { .. } => CastEvent::StreamStarted,
                ReceiverEvent::PhoneDisconnected { .. } => CastEvent::PhoneDisconnected,
                ReceiverEvent::PlaybackFailed { message } => CastEvent::PlaybackFailed(message),
                ReceiverEvent::Status(
                    status @ (Listening::NotListening
                    | Listening::DisabledByPolicy
                    | Listening::TemporarilyDisabled),
                ) => CastEvent::Lost(format!("Windows stopped the receiver ({status:?}).")),
                ReceiverEvent::Status(_) => return,
            };
            events(event);
        };
        match Receiver::start(&ReceiverOptions::default(), listener) {
            Ok(receiver) => {
                let verdict = receiver.verdict();
                match verdict {
                    // Started but not (yet) listening: Windows usually catches
                    // up; the status event reports it if not.
                    Verdict::Ready | Verdict::Unknown => {
                        self.apply_volume(&receiver);
                        *lock(&self.receiver) = Some(receiver);
                        StartOutcome::Ready
                    }
                    Verdict::WifiCardCannotReceive
                    | Verdict::AccessDenied
                    | Verdict::DisabledByPolicy => {
                        StartOutcome::Unsupported(verdict.explain().to_owned())
                    }
                    Verdict::TemporarilyDisabled => {
                        StartOutcome::Failed(verdict.explain().to_owned())
                    }
                }
            }
            Err(failure) => match failure.verdict() {
                verdict @ (Verdict::WifiCardCannotReceive
                | Verdict::AccessDenied
                | Verdict::DisabledByPolicy) => {
                    log::warn!("Miracast receiver: {failure}");
                    StartOutcome::Unsupported(verdict.explain().to_owned())
                }
                _ => StartOutcome::Failed(failure.to_string()),
            },
        }
    }

    fn stop(&self) {
        if let Some(video) = lock(&self.video).as_ref() {
            video.detach();
        }
        lock(&self.receiver).take();
    }

    fn play(&self) {
        if let Some(receiver) = lock(&self.receiver).as_ref() {
            receiver.play();
        }
    }

    fn reject(&self) {
        if let Some(receiver) = lock(&self.receiver).as_ref() {
            receiver.reject();
        }
    }

    fn set_volume(&self, level: Level) {
        *lock(&self.volume) = Some(level);
        if let Some(receiver) = lock(&self.receiver).as_ref() {
            self.apply_volume(receiver);
        }
    }

    fn show_video(&self, visible: bool) {
        let player = lock(&self.receiver).as_ref().and_then(Receiver::player);
        let mut video = lock(&self.video);
        match (visible, player) {
            (true, Some(player)) => {
                if video.is_none() {
                    match VideoWindow::spawn() {
                        Ok(window) => *video = Some(window),
                        Err(err) => {
                            log::warn!("couldn't open the video window: {err}");
                            return;
                        }
                    }
                }
                if let Some(window) = video.as_ref() {
                    window.attach(player, true);
                }
            }
            _ => {
                if let Some(window) = video.as_ref() {
                    window.detach();
                }
            }
        }
    }
}
