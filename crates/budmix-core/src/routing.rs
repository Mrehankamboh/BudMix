//! Laptop audio in the buds: makes the connected buds the default playback
//! device while BudMix is active, and gives the user's previous default
//! back when the buds go away or the feature is turned off.
//!
//! [`OutputRouter`] is the pure decision logic; [`AudioBackend`] is the OS
//! boundary that [`crate::audio_runtime`] drives.

use serde::Serialize;

use crate::backend::{BackendError, ChangeNotifier, Watch};
use crate::controller::DeviceView;
use crate::device::{ConnectionState, DeviceId};
use crate::volume::Level;

/// A playback endpoint as Windows names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub id: String,
    pub name: String,
}

impl Endpoint {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
        }
    }
}

/// The audio side of the operating system.
pub trait AudioBackend: Send + Sync {
    /// The default playback endpoint for media, if there is one.
    fn default_output(&self) -> Result<Option<Endpoint>, BackendError>;
    /// The stereo playback endpoint of these buds, if they are connected.
    fn buds_output(&self, buds: &DeviceId) -> Result<Option<Endpoint>, BackendError>;
    /// Makes `endpoint_id` the default playback device for every role.
    fn set_default_output(&self, endpoint_id: &str) -> Result<(), BackendError>;
    /// True while an app records from the buds' microphone. Classic
    /// Bluetooth buds then switch to the hands-free (call) profile and every
    /// sound becomes mono and low quality.
    fn call_mode(&self, buds: &DeviceId) -> Result<bool, BackendError>;
    /// Plays a short beep on `endpoint_id` (or the default output) and
    /// returns when it has finished.
    fn play_tone(&self, endpoint_id: Option<&str>) -> Result<(), BackendError>;
    /// Sets the buds' own volume (the master slider).
    fn set_master_volume(&self, _endpoint_id: &str, _level: Level) -> Result<(), BackendError> {
        Ok(())
    }
    /// Applies `laptop` to every other app playing on the endpoint and `own`
    /// to BudMix's own audio session (the phone).
    fn apply_session_volumes(
        &self,
        _endpoint_id: &str,
        _laptop: Level,
        _own: Level,
    ) -> Result<(), BackendError> {
        Ok(())
    }
    /// Gives other apps the volume they had before BudMix changed it.
    fn restore_session_volumes(&self, _endpoint_id: &str) -> Result<(), BackendError> {
        Ok(())
    }
    /// Calls `notify` when playback devices or the default device change.
    fn watch(&self, notify: ChangeNotifier) -> Result<Watch, BackendError>;
}

/// The buds laptop audio goes to: the preferred device when it is connected,
/// otherwise the first connected one.
pub fn route_target(views: &[DeviceView], preferred: Option<&DeviceId>) -> Option<DeviceId> {
    let connected = |view: &&DeviceView| view.state == ConnectionState::Connected;
    views
        .iter()
        .filter(connected)
        .find(|view| Some(&view.id) == preferred)
        .or_else(|| views.iter().find(connected))
        .map(|view| view.id.clone())
}

/// What the router wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteAction {
    /// Make the buds the default output.
    UseBuds(String),
    /// Give the user's previous default back.
    Restore(String),
}

/// Decides when to switch the default output to the buds and back.
///
/// It never fights the user: if they pick another output while the buds are
/// connected, BudMix leaves it alone until the buds reconnect.
#[derive(Debug, Clone, Default)]
pub struct OutputRouter {
    enabled: bool,
    /// The default output before BudMix switched to the buds.
    saved: Option<String>,
    /// The buds endpoint BudMix has already routed to (or found as the
    /// default) during the current connection.
    routed: Option<String>,
}

impl OutputRouter {
    /// `saved` is a previous default remembered from an earlier run, so it
    /// can still be restored after a crash.
    pub fn new(enabled: bool, saved: Option<String>) -> Self {
        Self {
            enabled,
            saved,
            routed: None,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// The output to give back later, to persist across restarts.
    pub fn saved(&self) -> Option<&str> {
        self.saved.as_deref()
    }

    /// Called with the current state whenever something may have changed.
    pub fn update(&mut self, buds: Option<&str>, current: Option<&str>) -> Option<RouteAction> {
        let Some(buds) = buds else {
            // Buds gone: hand the previous default back.
            self.routed = None;
            return self.take_restore(current);
        };
        if !self.enabled {
            return None;
        }
        if self.routed.as_deref() == Some(buds) {
            // Already routed for this connection. If the default is now
            // something else, the user chose it.
            return None;
        }
        self.routed = Some(buds.to_owned());
        if current == Some(buds) {
            return None;
        }
        if self.saved.is_none() {
            self.saved = current.map(str::to_owned);
        }
        Some(RouteAction::UseBuds(buds.to_owned()))
    }

    /// Turns the feature on or off. Turning it off restores the previous
    /// default if the buds are still the default.
    pub fn set_enabled(
        &mut self,
        enabled: bool,
        buds: Option<&str>,
        current: Option<&str>,
    ) -> Option<RouteAction> {
        self.enabled = enabled;
        self.routed = None;
        if enabled {
            return self.update(buds, current);
        }
        self.release(buds, current)
    }

    /// BudMix is quitting: give the previous default back if the buds are
    /// still the default.
    pub fn shutdown(&mut self, buds: Option<&str>, current: Option<&str>) -> Option<RouteAction> {
        self.routed = None;
        self.release(buds, current)
    }

    fn release(&mut self, buds: Option<&str>, current: Option<&str>) -> Option<RouteAction> {
        if buds.is_some() && current != buds {
            // The user already moved away from the buds; keep their choice.
            self.saved = None;
            return None;
        }
        self.take_restore(current)
    }

    fn take_restore(&mut self, current: Option<&str>) -> Option<RouteAction> {
        let saved = self.saved.take()?;
        (current != Some(saved.as_str())).then_some(RouteAction::Restore(saved))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUDS: &str = "buds";
    const SPEAKERS: &str = "speakers";
    const HDMI: &str = "hdmi";

    fn use_buds() -> Option<RouteAction> {
        Some(RouteAction::UseBuds(BUDS.into()))
    }

    fn restore(id: &str) -> Option<RouteAction> {
        Some(RouteAction::Restore(id.into()))
    }

    #[test]
    fn switches_to_buds_and_remembers_the_previous_default() {
        let mut router = OutputRouter::new(true, None);
        assert_eq!(router.update(Some(BUDS), Some(SPEAKERS)), use_buds());
        assert_eq!(router.saved(), Some(SPEAKERS));
        // Once Windows reports the buds as default, nothing more to do.
        assert_eq!(router.update(Some(BUDS), Some(BUDS)), None);
    }

    #[test]
    fn restores_when_the_buds_disconnect() {
        let mut router = OutputRouter::new(true, None);
        router.update(Some(BUDS), Some(SPEAKERS));
        assert_eq!(router.update(None, Some(HDMI)), restore(SPEAKERS));
        assert_eq!(router.saved(), None);
        // Reconnect routes again and remembers the restored default.
        assert_eq!(router.update(Some(BUDS), Some(SPEAKERS)), use_buds());
        assert_eq!(router.saved(), Some(SPEAKERS));
    }

    #[test]
    fn no_restore_when_windows_already_fell_back_to_it() {
        let mut router = OutputRouter::new(true, None);
        router.update(Some(BUDS), Some(SPEAKERS));
        assert_eq!(router.update(None, Some(SPEAKERS)), None);
        assert_eq!(router.saved(), None);
    }

    #[test]
    fn buds_already_default_needs_no_switch_and_nothing_to_restore() {
        let mut router = OutputRouter::new(true, None);
        assert_eq!(router.update(Some(BUDS), Some(BUDS)), None);
        assert_eq!(router.saved(), None);
        assert_eq!(router.update(None, Some(SPEAKERS)), None);
    }

    #[test]
    fn never_fights_a_manual_choice() {
        let mut router = OutputRouter::new(true, None);
        router.update(Some(BUDS), Some(SPEAKERS));
        router.update(Some(BUDS), Some(BUDS));
        // The user picks HDMI while the buds stay connected.
        assert_eq!(router.update(Some(BUDS), Some(HDMI)), None);
        assert_eq!(router.update(Some(BUDS), Some(HDMI)), None);
    }

    #[test]
    fn disabled_does_nothing_but_still_restores_a_saved_default() {
        let mut router = OutputRouter::new(false, Some(SPEAKERS.into()));
        assert_eq!(router.update(Some(BUDS), Some(HDMI)), None);
        assert_eq!(router.update(None, Some(HDMI)), restore(SPEAKERS));
    }

    #[test]
    fn turning_off_restores_only_if_the_buds_are_still_default() {
        let mut router = OutputRouter::new(true, None);
        router.update(Some(BUDS), Some(SPEAKERS));
        assert_eq!(
            router.set_enabled(false, Some(BUDS), Some(BUDS)),
            restore(SPEAKERS)
        );

        let mut router = OutputRouter::new(true, None);
        router.update(Some(BUDS), Some(SPEAKERS));
        assert_eq!(router.set_enabled(false, Some(BUDS), Some(HDMI)), None);
        assert_eq!(router.saved(), None);
    }

    #[test]
    fn turning_on_routes_immediately() {
        let mut router = OutputRouter::new(false, None);
        assert_eq!(
            router.set_enabled(true, Some(BUDS), Some(SPEAKERS)),
            use_buds()
        );
    }

    #[test]
    fn turning_on_again_reroutes_after_a_manual_choice() {
        let mut router = OutputRouter::new(true, None);
        router.update(Some(BUDS), Some(SPEAKERS));
        router.update(Some(BUDS), Some(HDMI));
        router.set_enabled(false, Some(BUDS), Some(HDMI));
        assert_eq!(router.set_enabled(true, Some(BUDS), Some(HDMI)), use_buds());
        assert_eq!(router.saved(), Some(HDMI));
    }

    #[test]
    fn shutdown_restores_and_forgets() {
        let mut router = OutputRouter::new(true, None);
        router.update(Some(BUDS), Some(SPEAKERS));
        assert_eq!(router.shutdown(Some(BUDS), Some(BUDS)), restore(SPEAKERS));
        assert_eq!(router.saved(), None);
    }

    #[test]
    fn a_saved_default_from_a_crash_is_restored_once() {
        let mut router = OutputRouter::new(true, Some(SPEAKERS.into()));
        // The buds are the default after the restart; keep the old saved value.
        assert_eq!(router.update(Some(BUDS), Some(BUDS)), None);
        assert_eq!(router.saved(), Some(SPEAKERS));
        assert_eq!(router.update(None, Some(HDMI)), restore(SPEAKERS));
    }

    fn device(id: &str, state: ConnectionState) -> DeviceView {
        DeviceView {
            id: DeviceId::new(id),
            name: id.to_owned(),
            profiles: Vec::new(),
            state,
            reconnect_attempt: None,
            last_error: None,
            last_connected: false,
        }
    }

    #[test]
    fn route_target_prefers_the_preferred_connected_buds() {
        let views = [
            device("a", ConnectionState::Connected),
            device("b", ConnectionState::Connected),
        ];
        let b = DeviceId::new("b");
        assert_eq!(route_target(&views, Some(&b)), Some(b));
        assert_eq!(route_target(&views, None), Some(DeviceId::new("a")));
    }

    #[test]
    fn route_target_skips_buds_that_are_not_connected() {
        let views = [
            device("a", ConnectionState::Connecting),
            device("b", ConnectionState::Connected),
        ];
        let a = DeviceId::new("a");
        assert_eq!(route_target(&views, Some(&a)), Some(DeviceId::new("b")));
        let views = [device("a", ConnectionState::Disconnected)];
        assert_eq!(route_target(&views, None), None);
    }

    #[test]
    fn no_default_output_at_all_is_handled() {
        let mut router = OutputRouter::new(true, None);
        assert_eq!(router.update(Some(BUDS), None), use_buds());
        assert_eq!(router.saved(), None);
        assert_eq!(router.update(None, None), None);
    }
}
