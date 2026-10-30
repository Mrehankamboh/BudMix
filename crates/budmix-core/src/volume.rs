//! Per-source volume: master (the buds), laptop apps, phone over Wi-Fi and
//! phone over Bluetooth. Levels are whole percents so settings stay exact.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Level {
    /// 0 to 100.
    pub percent: u8,
    pub muted: bool,
}

impl Level {
    pub const FULL: Self = Self {
        percent: 100,
        muted: false,
    };

    /// The gain to apply, 0.0 to 1.0 (0 when muted).
    pub fn gain(self) -> f32 {
        if self.muted {
            0.0
        } else {
            f32::from(self.percent.min(100)) / 100.0
        }
    }
}

impl Default for Level {
    fn default() -> Self {
        Self::FULL
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VolumeSource {
    Master,
    Laptop,
    PhoneWifi,
    PhoneBluetooth,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Volumes {
    /// The buds' own volume.
    pub master: Level,
    /// Every other app on the laptop.
    pub laptop: Level,
    pub phone_wifi: Level,
    pub phone_bluetooth: Level,
}

impl Volumes {
    pub fn get(&self, source: VolumeSource) -> Level {
        match source {
            VolumeSource::Master => self.master,
            VolumeSource::Laptop => self.laptop,
            VolumeSource::PhoneWifi => self.phone_wifi,
            VolumeSource::PhoneBluetooth => self.phone_bluetooth,
        }
    }

    pub fn set(&mut self, source: VolumeSource, level: Level) {
        let level = Level {
            percent: level.percent.min(100),
            ..level
        };
        match source {
            VolumeSource::Master => self.master = level,
            VolumeSource::Laptop => self.laptop = level,
            VolumeSource::PhoneWifi => self.phone_wifi = level,
            VolumeSource::PhoneBluetooth => self.phone_bluetooth = level,
        }
    }

    pub fn sanitized(mut self) -> Self {
        for source in [
            VolumeSource::Master,
            VolumeSource::Laptop,
            VolumeSource::PhoneWifi,
            VolumeSource::PhoneBluetooth,
        ] {
            let level = self.get(source);
            self.set(source, level);
        }
        self
    }

    /// BudMix's own audio session carries the Bluetooth phone stream (and
    /// the cast, which has its own player volume on top). While the
    /// Bluetooth phone plays, the session gets its level; otherwise full, so
    /// the cast is controlled by its own slider alone.
    pub fn own_session(&self, bluetooth_playing: bool) -> Level {
        if bluetooth_playing {
            self.phone_bluetooth
        } else {
            Level::FULL
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_follows_percent_and_mute() {
        assert_eq!(Level::FULL.gain(), 1.0);
        assert_eq!(
            Level {
                percent: 50,
                muted: false
            }
            .gain(),
            0.5
        );
        assert_eq!(
            Level {
                percent: 80,
                muted: true
            }
            .gain(),
            0.0
        );
        assert_eq!(
            Level {
                percent: 250,
                muted: false
            }
            .gain(),
            1.0
        );
    }

    #[test]
    fn set_clamps_and_get_reads_back() {
        let mut volumes = Volumes::default();
        volumes.set(
            VolumeSource::Laptop,
            Level {
                percent: 140,
                muted: false,
            },
        );
        assert_eq!(volumes.get(VolumeSource::Laptop).percent, 100);
        volumes.set(
            VolumeSource::PhoneWifi,
            Level {
                percent: 30,
                muted: true,
            },
        );
        assert_eq!(
            volumes.phone_wifi,
            Level {
                percent: 30,
                muted: true
            }
        );
        assert_eq!(volumes.master, Level::FULL);
    }

    #[test]
    fn own_session_follows_the_bluetooth_phone_only_while_it_plays() {
        let mut volumes = Volumes::default();
        volumes.set(
            VolumeSource::PhoneBluetooth,
            Level {
                percent: 40,
                muted: false,
            },
        );
        assert_eq!(volumes.own_session(true).percent, 40);
        assert_eq!(volumes.own_session(false), Level::FULL);
    }

    #[test]
    fn sanitizing_fixes_out_of_range_values_from_a_file() {
        let volumes: Volumes = serde_json::from_str(
            r#"{ "master": { "percent": 255 }, "laptop": { "muted": true } }"#,
        )
        .unwrap();
        let volumes = volumes.sanitized();
        assert_eq!(volumes.master.percent, 100);
        assert_eq!(
            volumes.laptop,
            Level {
                percent: 100,
                muted: true
            }
        );
        assert_eq!(volumes.phone_bluetooth, Level::FULL);
    }
}
