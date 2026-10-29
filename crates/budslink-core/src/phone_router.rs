//! Which way the phone's audio comes in: Wi-Fi (cast), Bluetooth, or Auto.
//!
//! The rule that matters: the same phone must never play twice (cast and
//! Bluetooth at once would echo). While the phone casts, the Bluetooth link
//! is held off; a few seconds after the cast ends it comes back in Auto mode.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhoneMode {
    /// Wi-Fi while the phone casts, Bluetooth otherwise.
    #[default]
    Auto,
    WifiOnly,
    BluetoothOnly,
}

impl PhoneMode {
    /// The Miracast receiver runs in this mode.
    pub fn uses_wifi(self) -> bool {
        self != PhoneMode::BluetoothOnly
    }
}

/// How long after a cast ends Bluetooth waits before coming back, so a phone
/// that drops and recasts doesn't bounce between the two.
pub const CAST_GRACE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct PhoneRouter {
    mode: PhoneMode,
    casting: bool,
    cast_ended: Option<Instant>,
}

impl PhoneRouter {
    pub fn new(mode: PhoneMode) -> Self {
        Self {
            mode,
            casting: false,
            cast_ended: None,
        }
    }

    pub fn mode(&self) -> PhoneMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: PhoneMode) {
        self.mode = mode;
    }

    pub fn set_casting(&mut self, casting: bool, now: Instant) {
        if self.casting && !casting {
            self.cast_ended = Some(now);
        }
        self.casting = casting;
    }

    /// True when the Bluetooth phone link must stay off.
    pub fn bluetooth_suspended(&self, now: Instant) -> bool {
        match self.mode {
            PhoneMode::WifiOnly => true,
            PhoneMode::BluetoothOnly => false,
            PhoneMode::Auto => {
                self.casting
                    || self
                        .cast_ended
                        .is_some_and(|ended| now < ended + CAST_GRACE)
            }
        }
    }

    /// When the answer of [`Self::bluetooth_suspended`] changes by itself.
    pub fn next_wakeup(&self, now: Instant) -> Option<Instant> {
        if self.mode != PhoneMode::Auto || self.casting {
            return None;
        }
        self.cast_ended
            .map(|ended| ended + CAST_GRACE)
            .filter(|at| *at > now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_holds_bluetooth_off_while_casting_and_a_moment_after() {
        let now = Instant::now();
        let mut router = PhoneRouter::new(PhoneMode::Auto);
        assert!(!router.bluetooth_suspended(now));
        router.set_casting(true, now);
        assert!(router.bluetooth_suspended(now));
        assert_eq!(router.next_wakeup(now), None);

        let ended = now + Duration::from_secs(60);
        router.set_casting(false, ended);
        assert!(router.bluetooth_suspended(ended + Duration::from_secs(2)));
        assert_eq!(router.next_wakeup(ended), Some(ended + CAST_GRACE));
        assert!(!router.bluetooth_suspended(ended + CAST_GRACE));
        assert_eq!(router.next_wakeup(ended + CAST_GRACE), None);
    }

    #[test]
    fn fixed_modes_ignore_casting() {
        let now = Instant::now();
        let mut wifi = PhoneRouter::new(PhoneMode::WifiOnly);
        assert!(wifi.bluetooth_suspended(now));
        wifi.set_casting(true, now);
        wifi.set_casting(false, now);
        assert!(wifi.bluetooth_suspended(now + Duration::from_secs(60)));

        let mut bluetooth = PhoneRouter::new(PhoneMode::BluetoothOnly);
        bluetooth.set_casting(true, now);
        assert!(!bluetooth.bluetooth_suspended(now));
        assert_eq!(bluetooth.next_wakeup(now), None);
    }

    #[test]
    fn only_bluetooth_mode_turns_the_receiver_off() {
        assert!(PhoneMode::Auto.uses_wifi());
        assert!(PhoneMode::WifiOnly.uses_wifi());
        assert!(!PhoneMode::BluetoothOnly.uses_wifi());
    }

    #[test]
    fn switching_mode_applies_at_once() {
        let now = Instant::now();
        let mut router = PhoneRouter::new(PhoneMode::BluetoothOnly);
        router.set_casting(true, now);
        router.set_mode(PhoneMode::Auto);
        assert!(router.bluetooth_suspended(now));
        assert_eq!(router.mode(), PhoneMode::Auto);
    }

    #[test]
    fn mode_serializes_in_camel_case() {
        assert_eq!(
            serde_json::to_string(&PhoneMode::WifiOnly).unwrap(),
            "\"wifiOnly\""
        );
    }
}
