//! Exponential back-off used when buds drop unexpectedly.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// How BudsLink retries after an unexpected disconnect.
///
/// Attempt `n` (1-based) waits `initial_delay * 2^(n-1)`, capped at
/// `max_delay`. After `max_attempts` failed attempts it gives up, so the
/// laptop never keeps fighting the phone for the buds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReconnectPolicy {
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_attempts: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_delay_ms: 1_000,
            max_delay_ms: 30_000,
            max_attempts: 8,
        }
    }
}

impl ReconnectPolicy {
    pub const MIN_DELAY_MS: u64 = 250;
    pub const MAX_DELAY_MS: u64 = 10 * 60 * 1_000;
    pub const MAX_ATTEMPTS: u32 = 100;

    /// Delay before attempt `attempt` (1-based), or `None` once the policy is exhausted.
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        if attempt == 0 || attempt > self.max_attempts {
            return None;
        }
        let factor = 1u64.checked_shl(attempt - 1).unwrap_or(u64::MAX);
        let delay = self.initial_delay_ms.saturating_mul(factor);
        Some(Duration::from_millis(delay.min(self.max_delay_ms)))
    }

    /// Clamps user-provided values into a sane range.
    pub fn sanitized(self) -> Self {
        let initial = self
            .initial_delay_ms
            .clamp(Self::MIN_DELAY_MS, Self::MAX_DELAY_MS);
        Self {
            initial_delay_ms: initial,
            max_delay_ms: self.max_delay_ms.clamp(initial, Self::MAX_DELAY_MS),
            max_attempts: self.max_attempts.min(Self::MAX_ATTEMPTS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Option<Duration> {
        Some(Duration::from_millis(n))
    }

    #[test]
    fn doubles_until_capped() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.delay_for_attempt(1), ms(1_000));
        assert_eq!(policy.delay_for_attempt(2), ms(2_000));
        assert_eq!(policy.delay_for_attempt(3), ms(4_000));
        assert_eq!(policy.delay_for_attempt(5), ms(16_000));
        assert_eq!(policy.delay_for_attempt(6), ms(30_000));
        assert_eq!(policy.delay_for_attempt(8), ms(30_000));
    }

    #[test]
    fn gives_up_after_max_attempts() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.delay_for_attempt(9), None);
        assert_eq!(policy.delay_for_attempt(0), None);
    }

    #[test]
    fn zero_attempts_disables_retries() {
        let policy = ReconnectPolicy {
            max_attempts: 0,
            ..Default::default()
        };
        assert_eq!(policy.delay_for_attempt(1), None);
    }

    #[test]
    fn huge_attempt_numbers_do_not_overflow() {
        let policy = ReconnectPolicy {
            max_attempts: u32::MAX,
            ..Default::default()
        };
        assert_eq!(policy.delay_for_attempt(64), ms(30_000));
        assert_eq!(policy.delay_for_attempt(u32::MAX), ms(30_000));
    }

    #[test]
    fn sanitized_clamps_values() {
        let policy = ReconnectPolicy {
            initial_delay_ms: 1,
            max_delay_ms: 0,
            max_attempts: 10_000,
        }
        .sanitized();
        assert_eq!(policy.initial_delay_ms, ReconnectPolicy::MIN_DELAY_MS);
        assert_eq!(policy.max_delay_ms, ReconnectPolicy::MIN_DELAY_MS);
        assert_eq!(policy.max_attempts, ReconnectPolicy::MAX_ATTEMPTS);
    }
}
