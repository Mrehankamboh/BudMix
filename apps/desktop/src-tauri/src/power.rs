//! Notices when the PC wakes from sleep and refreshes everything, so the
//! buds, the phone and the receiver come back without a click.

use std::thread;
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Manager};

use crate::state::AppState;

const TICK: Duration = Duration::from_secs(5);
/// A tick that took this much longer than expected means the PC slept.
const SLEEP_GAP: Duration = Duration::from_secs(20);

/// True when the wall clock moved much further than one tick.
fn slept(before: SystemTime, after: SystemTime) -> bool {
    after
        .duration_since(before)
        .is_ok_and(|elapsed| elapsed > TICK + SLEEP_GAP)
}

pub fn watch_for_wake(app: AppHandle) {
    let spawned = thread::Builder::new()
        .name("budmix-wake".into())
        .spawn(move || {
            let mut before = SystemTime::now();
            loop {
                thread::sleep(TICK);
                let now = SystemTime::now();
                if slept(before, now) {
                    log::info!("woke from sleep; refreshing devices, phone and receiver");
                    if let Some(state) = app.try_state::<AppState>() {
                        state.runtime().refresh();
                        state.bridge().refresh();
                        state.audio().refresh();
                        state.cast().check_network();
                    }
                }
                before = now;
            }
        });
    if let Err(err) = spawned {
        log::warn!("can't watch for wake-ups: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_gap_means_the_pc_slept() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        assert!(!slept(start, start + TICK));
        assert!(!slept(start, start + TICK + Duration::from_secs(5)));
        assert!(slept(start, start + Duration::from_secs(600)));
        // A clock set backwards is not a wake-up.
        assert!(!slept(start, start - Duration::from_secs(600)));
    }
}
