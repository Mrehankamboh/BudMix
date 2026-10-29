//! Bridge mode on Windows: the laptop acts as a Bluetooth speaker for a
//! paired phone through `Windows.Media.Audio.AudioPlaybackConnection`
//! (Windows 10 2004+). The phone's audio plays on the default output, which is
//! the buds when they are connected, mixed with everything else Windows plays.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use budslink_core::{
    AudioSource, BackendError, ChangeNotifier, DeviceId, SourceBackend, UnavailableSource, Watch,
};
use windows::Devices::Enumeration::{DeviceInformation, DeviceInformationUpdate, DeviceWatcher};
use windows::Foundation::TypedEventHandler;
use windows::Media::Audio::{
    AudioPlaybackConnection, AudioPlaybackConnectionOpenResultStatus, AudioPlaybackConnectionState,
};
use windows::core::{HSTRING, IInspectable};

use crate::{bluetooth, com};

struct Connection {
    connection: AudioPlaybackConnection,
    state_token: i64,
}

impl Connection {
    fn is_open(&self) -> bool {
        self.connection
            .State()
            .is_ok_and(|state| state == AudioPlaybackConnectionState::Opened)
    }

    fn close(self) {
        let _ = self.connection.RemoveStateChanged(self.state_token);
        if let Err(err) = self.connection.Close() {
            log::warn!("closing phone audio failed: {err}");
        }
    }
}

type Notifier = Arc<Mutex<Option<ChangeNotifier>>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn notify(notifier: &Notifier) {
    let callback = lock(notifier).clone();
    if let Some(callback) = callback {
        callback();
    }
}

/// [`SourceBackend`] backed by `AudioPlaybackConnection`.
#[derive(Default)]
pub struct PhoneAudioBackend {
    connections: Mutex<HashMap<DeviceId, Connection>>,
    notifier: Notifier,
}

impl PhoneAudioBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn selector() -> Result<HSTRING, BackendError> {
        com::ensure_initialized();
        AudioPlaybackConnection::GetDeviceSelector().map_err(os_error)
    }
}

impl SourceBackend for PhoneAudioBackend {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn list_sources(&self) -> Result<Vec<AudioSource>, BackendError> {
        let selector = Self::selector()?;
        let devices = DeviceInformation::FindAllAsyncAqsFilter(&selector)
            .and_then(|operation| operation.join())
            .map_err(os_error)?;
        let connections = lock(&self.connections);
        let mut sources = Vec::new();
        for device in &devices {
            let (Ok(id), Ok(name)) = (device.Id(), device.Name()) else {
                continue;
            };
            let id = DeviceId::new(id.to_string());
            let streaming = connections.get(&id).is_some_and(Connection::is_open);
            sources.push(AudioSource {
                id,
                name: name.to_string(),
                streaming,
            });
        }
        Ok(sources)
    }

    fn unavailable(&self) -> Vec<UnavailableSource> {
        match Self::selector() {
            Ok(selector) => bluetooth::unavailable_phones(&selector.to_string()),
            Err(err) => {
                log::warn!("phone audio is unavailable: {err}");
                Vec::new()
            }
        }
    }

    fn diagnostics(&self) -> String {
        match Self::selector() {
            Ok(selector) => bluetooth::report(&selector.to_string()),
            Err(err) => format!("Phone audio isn't available on this Windows: {err}"),
        }
    }

    fn open(&self, id: &DeviceId) -> Result<(), BackendError> {
        com::ensure_initialized();
        let existing = lock(&self.connections).remove(id);
        if let Some(existing) = existing {
            if existing.is_open() {
                lock(&self.connections).insert(id.clone(), existing);
                return Ok(());
            }
            existing.close();
        }

        let connection = AudioPlaybackConnection::TryCreateFromId(&HSTRING::from(id.as_str()))
            .map_err(|_| BackendError::NotFound(id.clone()))?;
        let notifier = Arc::clone(&self.notifier);
        let state_token = connection
            .StateChanged(
                &TypedEventHandler::<AudioPlaybackConnection, IInspectable>::new(move |_, _| {
                    notify(&notifier);
                    Ok(())
                }),
            )
            .map_err(os_error)?;
        let connection = Connection {
            connection,
            state_token,
        };

        // Start makes the laptop available as a speaker for this phone; Open
        // asks the phone to start streaming to it.
        let outcome = connection
            .connection
            .Start()
            .and_then(|()| connection.connection.Open())
            .and_then(|result| Ok((result.Status()?, result.ExtendedError()?)));
        match outcome {
            Ok((AudioPlaybackConnectionOpenResultStatus::Success, _)) => {
                lock(&self.connections).insert(id.clone(), connection);
                Ok(())
            }
            Ok((status, extended)) => {
                connection.close();
                Err(open_error(status, extended.0))
            }
            Err(err) => {
                connection.close();
                Err(os_error(err))
            }
        }
    }

    fn close(&self, id: &DeviceId) -> Result<(), BackendError> {
        if let Some(connection) = lock(&self.connections).remove(id) {
            connection.close();
        }
        notify(&self.notifier);
        Ok(())
    }

    fn watch(&self, notify_change: ChangeNotifier) -> Result<Watch, BackendError> {
        *lock(&self.notifier) = Some(notify_change);
        let selector = Self::selector()?;
        let watcher = DeviceInformation::CreateWatcherAqsFilter(&selector).map_err(os_error)?;

        let added = Arc::clone(&self.notifier);
        let updated = Arc::clone(&self.notifier);
        let removed = Arc::clone(&self.notifier);
        watcher
            .Added(&TypedEventHandler::<DeviceWatcher, DeviceInformation>::new(
                move |_, _| {
                    notify(&added);
                    Ok(())
                },
            ))
            .and_then(|_| {
                watcher.Updated(
                    &TypedEventHandler::<DeviceWatcher, DeviceInformationUpdate>::new(
                        move |_, _| {
                            notify(&updated);
                            Ok(())
                        },
                    ),
                )
            })
            .and_then(|_| {
                watcher.Removed(
                    &TypedEventHandler::<DeviceWatcher, DeviceInformationUpdate>::new(
                        move |_, _| {
                            notify(&removed);
                            Ok(())
                        },
                    ),
                )
            })
            .and_then(|_| watcher.Start())
            .map_err(os_error)?;

        Ok(Watch::new(WatcherGuard {
            watcher,
            notifier: Arc::clone(&self.notifier),
        }))
    }
}

impl Drop for PhoneAudioBackend {
    fn drop(&mut self) {
        for (_, connection) in lock(&self.connections).drain() {
            connection.close();
        }
    }
}

struct WatcherGuard {
    watcher: DeviceWatcher,
    notifier: Notifier,
}

impl Drop for WatcherGuard {
    fn drop(&mut self) {
        let _ = self.watcher.Stop();
        *lock(&self.notifier) = None;
    }
}

/// `HRESULT_FROM_WIN32(ERROR_GEN_FAILURE)`: "A device attached to the system
/// is not functioning". Seen with phones that had worked before, too.
const GEN_FAILURE: i32 = 0x8007_001F_u32 as i32;

fn open_error(status: AudioPlaybackConnectionOpenResultStatus, extended: i32) -> BackendError {
    let message = match status {
        _ if extended == GEN_FAILURE => return driver_failure(),
        AudioPlaybackConnectionOpenResultStatus::RequestTimedOut => {
            "The phone didn't answer. Is its Bluetooth on and is it nearby?".to_owned()
        }
        AudioPlaybackConnectionOpenResultStatus::DeniedBySystem => {
            "Windows refused the connection. The phone may be playing to another device.".to_owned()
        }
        _ => format!("Couldn't receive audio from the phone (error {extended:#010x})."),
    };
    BackendError::Other(message)
}

fn driver_failure() -> BackendError {
    BackendError::Other(
        "Windows' Bluetooth driver couldn't start audio from this phone (error 0x8007001f). \
         Try turning Bluetooth off and on, on the laptop and on the phone."
            .to_owned(),
    )
}

fn os_error(err: windows::core::Error) -> BackendError {
    if err.code().0 == GEN_FAILURE {
        return driver_failure();
    }
    BackendError::Os {
        code: err.code().0,
        message: err.message().to_string(),
    }
}
