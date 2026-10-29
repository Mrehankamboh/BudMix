//! The Miracast receiver on Windows.
//!
//! Flow, as the `Windows.Media.Miracast` docs require: apply settings, create
//! a session, subscribe to its events, then start it. When a phone connects,
//! Windows hands over a `MediaSource` with the cast's audio and video; a
//! `MediaPlayer` plays it, so the audio goes to the default output.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use windows::ApplicationModel::Core::CoreApplicationView;
use windows::Foundation::TypedEventHandler;
use windows::Media::Core::MediaSource;
use windows::Media::Miracast::{
    MiracastReceiver, MiracastReceiverApplySettingsStatus, MiracastReceiverAuthorizationMethod,
    MiracastReceiverConnection, MiracastReceiverConnectionCreatedEventArgs,
    MiracastReceiverDisconnectedEventArgs, MiracastReceiverListeningStatus,
    MiracastReceiverMediaSourceCreatedEventArgs, MiracastReceiverSession,
    MiracastReceiverSessionStartStatus, MiracastReceiverSettings, MiracastReceiverWiFiStatus,
};
use windows::Media::Playback::{MediaPlayer, MediaPlayerAudioCategory, MediaPlayerFailedEventArgs};
use windows::Win32::Foundation::{E_ACCESSDENIED, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
use windows::core::{Error, HRESULT, HSTRING, IInspectable};

use crate::probe::{self, Listening, Outcome, ProbeResult, Verdict, WifiSupport};

/// How the receiver presents itself to phones.
#[derive(Debug, Clone)]
pub struct ReceiverOptions {
    /// The name phones show in their Cast list.
    pub friendly_name: String,
    /// Ask phones for a PIN shown on the laptop on every connection.
    pub require_pin: bool,
}

impl Default for ReceiverOptions {
    fn default() -> Self {
        Self {
            friendly_name: "BudsLink".to_owned(),
            require_pin: false,
        }
    }
}

/// Something that happened to the receiver after it started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverEvent {
    Status(Listening),
    PhoneConnected { name: String, pin: Option<String> },
    StreamStarted { name: String },
    PlaybackFailed { message: String },
    PhoneDisconnected { name: String },
}

/// The receiver could not start. `result` holds what each step reported.
#[derive(Debug, Clone)]
pub struct StartFailure {
    pub result: ProbeResult,
    pub step: &'static str,
    pub detail: String,
}

impl StartFailure {
    pub fn verdict(&self) -> Verdict {
        probe::verdict(&self.result)
    }
}

impl fmt::Display for StartFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "couldn't {}: {}", self.step, self.detail)
    }
}

impl std::error::Error for StartFailure {}

type Listener = Arc<dyn Fn(ReceiverEvent) + Send + Sync>;
type Players = Arc<Mutex<Vec<MediaPlayer>>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A started Miracast receiver. Dropping it stops the receiver and any
/// playback.
pub struct Receiver {
    receiver: MiracastReceiver,
    session: MiracastReceiverSession,
    status_token: Option<i64>,
    players: Players,
    result: ProbeResult,
}

impl Receiver {
    /// Applies the settings and starts listening for phones. `listener` is
    /// called from Windows' worker threads.
    pub fn start(
        options: &ReceiverOptions,
        listener: impl Fn(ReceiverEvent) + Send + Sync + 'static,
    ) -> Result<Self, StartFailure> {
        ensure_mta();
        let listener: Listener = Arc::new(listener);
        let mut result = ProbeResult::default();

        let receiver =
            MiracastReceiver::new().map_err(move |err| failure(result, "create the receiver", err))?;
        result.wifi = receiver
            .GetStatus()
            .and_then(|status| status.WiFiStatus())
            .ok()
            .map(wifi_support);

        let settings = receiver
            .GetDefaultSettings()
            .map_err(move |err| failure(result, "read the receiver settings", err))?;
        configure(&settings, options)
            .map_err(move |err| failure(result, "change the receiver settings", err))?;
        let applied = receiver
            .DisconnectAllAndApplySettings(&settings)
            .map_err(move |err| failure(result, "apply the receiver settings", err))?;
        let status = applied
            .Status()
            .map_err(move |err| failure(result, "apply the receiver settings", err))?;
        result.settings = Some(settings_outcome(status));
        if result.settings != Some(Outcome::Success) {
            return Err(StartFailure {
                result,
                step: "apply the receiver settings",
                detail: extended_error(applied.ExtendedError()),
            });
        }

        // A desktop app has no CoreApplicationView; the session is created
        // without one.
        let session = match receiver.CreateSession(None::<&CoreApplicationView>) {
            Ok(session) => session,
            Err(err) => {
                if err.code() == E_ACCESSDENIED {
                    result.start = Some(Outcome::AccessDenied);
                }
                return Err(failure(result, "create a receiver session", err));
            }
        };
        if let Err(err) = session.SetAllowConnectionTakeover(true) {
            log::debug!("connection takeover not available: {err}");
        }

        // Events must be subscribed before the session starts.
        let players: Players = Arc::default();
        if let Err(err) = subscribe(&session, &listener, &players) {
            let _ = session.Close();
            return Err(failure(result, "subscribe to receiver events", err));
        }

        let started = match session.Start() {
            Ok(started) => started,
            Err(err) => {
                let _ = session.Close();
                return Err(failure(result, "start the receiver session", err));
            }
        };
        result.start = Some(start_outcome(
            started.Status().unwrap_or(MiracastReceiverSessionStartStatus::UnknownFailure),
        ));
        if result.start != Some(Outcome::Success) {
            let _ = session.Close();
            return Err(StartFailure {
                result,
                step: "start the receiver session",
                detail: extended_error(started.ExtendedError()),
            });
        }

        // Starting is not the same as listening: read the status again.
        result.listening = current_listening(&receiver);
        let status_listener = Arc::clone(&listener);
        let status_token = receiver
            .StatusChanged(&TypedEventHandler::<MiracastReceiver, IInspectable>::new(
                move |sender, _| {
                    if let Some(listening) = sender.as_ref().and_then(current_listening) {
                        status_listener(ReceiverEvent::Status(listening));
                    }
                    Ok(())
                },
            ))
            .inspect_err(|err| log::warn!("can't watch the receiver status: {err}"))
            .ok();

        Ok(Self {
            receiver,
            session,
            status_token,
            players,
            result,
        })
    }

    /// What each start-up step reported.
    pub fn result(&self) -> ProbeResult {
        self.result
    }

    /// The verdict for the start-up result.
    pub fn verdict(&self) -> Verdict {
        probe::verdict(&self.result)
    }

    /// The receiver's listening status right now.
    pub fn listening(&self) -> Option<Listening> {
        current_listening(&self.receiver)
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        if let Some(token) = self.status_token {
            let _ = self.receiver.RemoveStatusChanged(token);
        }
        stop_players(&self.players);
        if let Err(err) = self.session.Close() {
            log::warn!("closing the receiver session failed: {err}");
        }
    }
}

fn configure(
    settings: &MiracastReceiverSettings,
    options: &ReceiverOptions,
) -> windows::core::Result<()> {
    settings.SetFriendlyName(&HSTRING::from(options.friendly_name.as_str()))?;
    settings.SetModelName(&HSTRING::from("BudsLink"))?;
    settings.SetAuthorizationMethod(if options.require_pin {
        MiracastReceiverAuthorizationMethod::PinDisplayRequired
    } else {
        MiracastReceiverAuthorizationMethod::None
    })?;
    settings.SetRequireAuthorizationFromKnownTransmitters(false)
}

fn subscribe(
    session: &MiracastReceiverSession,
    listener: &Listener,
    players: &Players,
) -> windows::core::Result<()> {
    let on_connect = Arc::clone(listener);
    session.ConnectionCreated(&TypedEventHandler::<
        MiracastReceiverSession,
        MiracastReceiverConnectionCreatedEventArgs,
    >::new(move |_, args| {
        let args = args.ok()?;
        let name = transmitter_name(&args.Connection()?);
        let pin = args
            .Pin()
            .ok()
            .map(|pin| pin.to_string())
            .filter(|pin| !pin.is_empty());
        on_connect(ReceiverEvent::PhoneConnected { name, pin });
        Ok(())
    }))?;

    let on_media = Arc::clone(listener);
    let media_players = Arc::clone(players);
    session.MediaSourceCreated(&TypedEventHandler::<
        MiracastReceiverSession,
        MiracastReceiverMediaSourceCreatedEventArgs,
    >::new(move |_, args| {
        let args = args.ok()?;
        let name = transmitter_name(&args.Connection()?);
        match play(&args.MediaSource()?, &on_media) {
            Ok(player) => {
                lock(&media_players).push(player);
                on_media(ReceiverEvent::StreamStarted { name });
            }
            Err(err) => on_media(ReceiverEvent::PlaybackFailed {
                message: err.message(),
            }),
        }
        Ok(())
    }))?;

    let on_disconnect = Arc::clone(listener);
    let disconnect_players = Arc::clone(players);
    session.Disconnected(&TypedEventHandler::<
        MiracastReceiverSession,
        MiracastReceiverDisconnectedEventArgs,
    >::new(move |_, args| {
        let name = args
            .ok()
            .and_then(|args| args.Connection())
            .map(|connection| transmitter_name(&connection))
            .unwrap_or_default();
        stop_players(&disconnect_players);
        on_disconnect(ReceiverEvent::PhoneDisconnected { name });
        Ok(())
    }))?;
    Ok(())
}

fn play(source: &MediaSource, listener: &Listener) -> windows::core::Result<MediaPlayer> {
    let player = MediaPlayer::new()?;
    // Media, never Communications: a communications stream makes Windows
    // switch Bluetooth buds to the low-quality hands-free profile.
    player.SetAudioCategory(MediaPlayerAudioCategory::Media)?;
    player.SetRealTimePlayback(true)?;
    let on_failed = Arc::clone(listener);
    player.MediaFailed(&TypedEventHandler::<MediaPlayer, MediaPlayerFailedEventArgs>::new(
        move |_, args| {
            let message = args
                .ok()
                .and_then(|args| args.ErrorMessage())
                .map(|message| message.to_string())
                .unwrap_or_default();
            on_failed(ReceiverEvent::PlaybackFailed { message });
            Ok(())
        },
    ))?;
    player.SetSource(source)?;
    player.Play()?;
    Ok(player)
}

fn stop_players(players: &Players) {
    for player in lock(players).drain(..) {
        if let Err(err) = player.Close() {
            log::warn!("closing a cast player failed: {err}");
        }
    }
}

fn transmitter_name(connection: &MiracastReceiverConnection) -> String {
    connection
        .Transmitter()
        .and_then(|transmitter| transmitter.Name())
        .map(|name| name.to_string())
        .unwrap_or_else(|_| "Unknown phone".to_owned())
}

fn current_listening(receiver: &MiracastReceiver) -> Option<Listening> {
    receiver
        .GetStatus()
        .and_then(|status| status.ListeningStatus())
        .ok()
        .map(listening)
}

fn failure(result: ProbeResult, step: &'static str, err: Error) -> StartFailure {
    StartFailure {
        result,
        step,
        detail: format!("{} ({:#010X})", err.message(), err.code().0),
    }
}

fn extended_error(code: windows::core::Result<HRESULT>) -> String {
    match code {
        Ok(code) if code.is_ok() => "Windows gave no further detail".to_owned(),
        Ok(code) => format!("{} ({:#010X})", Error::from_hresult(code).message(), code.0),
        Err(err) => format!("{} ({:#010X})", err.message(), err.code().0),
    }
}

fn ensure_mta() {
    // SAFETY: plain COM initialisation for the calling thread. It is left in
    // place for the thread's lifetime, which the WinRT objects need.
    let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if hr.is_err() && hr != RPC_E_CHANGED_MODE {
        log::warn!("CoInitializeEx failed: {hr:?}");
    }
}

fn wifi_support(status: MiracastReceiverWiFiStatus) -> WifiSupport {
    match status {
        MiracastReceiverWiFiStatus::MiracastSupported => WifiSupport::Supported,
        MiracastReceiverWiFiStatus::MiracastSupportNotOptimized => WifiSupport::NotOptimized,
        MiracastReceiverWiFiStatus::MiracastNotSupported => WifiSupport::NotSupported,
        _ => WifiSupport::Undetermined,
    }
}

fn settings_outcome(status: MiracastReceiverApplySettingsStatus) -> Outcome {
    match status {
        MiracastReceiverApplySettingsStatus::Success => Outcome::Success,
        MiracastReceiverApplySettingsStatus::MiracastNotSupported => Outcome::MiracastNotSupported,
        MiracastReceiverApplySettingsStatus::AccessDenied => Outcome::AccessDenied,
        MiracastReceiverApplySettingsStatus::UnknownFailure => Outcome::UnknownFailure,
        _ => Outcome::InvalidSettings,
    }
}

fn start_outcome(status: MiracastReceiverSessionStartStatus) -> Outcome {
    match status {
        MiracastReceiverSessionStartStatus::Success => Outcome::Success,
        MiracastReceiverSessionStartStatus::MiracastNotSupported => Outcome::MiracastNotSupported,
        MiracastReceiverSessionStartStatus::AccessDenied => Outcome::AccessDenied,
        _ => Outcome::UnknownFailure,
    }
}

fn listening(status: MiracastReceiverListeningStatus) -> Listening {
    match status {
        MiracastReceiverListeningStatus::Listening => Listening::Listening,
        MiracastReceiverListeningStatus::ConnectionPending => Listening::ConnectionPending,
        MiracastReceiverListeningStatus::Connected => Listening::Connected,
        MiracastReceiverListeningStatus::DisabledByPolicy => Listening::DisabledByPolicy,
        MiracastReceiverListeningStatus::TemporarilyDisabled => Listening::TemporarilyDisabled,
        _ => Listening::NotListening,
    }
}
