//! The boundary between BudMix's logic and the operating system.

use std::sync::Arc;

use thiserror::Error;

use crate::device::{AudioDevice, DeviceId};

/// Errors a backend can report.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BackendError {
    #[error("device {0} is not paired or no longer present")]
    NotFound(DeviceId),
    #[error("not supported: {0}")]
    NotSupported(String),
    #[error("system error {code:#010x}: {message}")]
    Os { code: i32, message: String },
    #[error("{0}")]
    Other(String),
}

/// Called by a backend, from any thread, whenever the device list or a
/// connection state may have changed. It must return quickly.
pub type ChangeNotifier = Arc<dyn Fn() + Send + Sync>;

/// Keeps a backend subscription alive; dropping it unsubscribes.
pub struct Watch(#[allow(dead_code)] Box<dyn Send>);

impl Watch {
    pub fn new(guard: impl Send + 'static) -> Self {
        Self(Box::new(guard))
    }

    /// A watch that holds nothing, for backends without change notifications.
    pub fn none() -> Self {
        Self::new(())
    }
}

/// Access to paired Bluetooth audio devices.
///
/// `connect` and `disconnect` only *request* a change: the result arrives
/// later through [`DeviceBackend::list_devices`] after the notifier fires.
pub trait DeviceBackend: Send + Sync {
    /// Short name for logs and the UI, e.g. `"windows"` or `"demo"`.
    fn name(&self) -> &'static str;

    fn list_devices(&self) -> Result<Vec<AudioDevice>, BackendError>;

    fn connect(&self, id: &DeviceId) -> Result<(), BackendError>;

    fn disconnect(&self, id: &DeviceId) -> Result<(), BackendError>;

    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError>;
}
