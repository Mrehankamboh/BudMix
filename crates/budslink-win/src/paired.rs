//! Works out which paired phones Windows won't stream audio from, so the UI
//! lists them with the reason instead of leaving them out. Pure, so it is
//! tested on every platform; the Windows queries live in `bluetooth`.

use budslink_core::{UnavailableReason, UnavailableSource};

/// Bluetooth Class of Device major class of phones.
pub const PHONE_MAJOR_CLASS: u32 = 2;
/// Bluetooth Low Energy appearance category of phones.
pub const PHONE_APPEARANCE_CATEGORY: u32 = 1;

/// A paired Bluetooth device as Windows lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    pub name: String,
    /// Device container, shared by everything Windows knows about one device.
    pub container: Option<String>,
    /// Paired over Bluetooth Low Energy rather than Classic.
    pub low_energy: bool,
    pub is_phone: bool,
}

/// A device Windows offers phone audio from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferedSource {
    pub name: String,
    pub container: Option<String>,
}

/// Same device: same container when both are known, otherwise the same name.
fn same_device(
    a_name: &str,
    a_container: Option<&str>,
    b_name: &str,
    b_container: Option<&str>,
) -> bool {
    match (a_container, b_container) {
        (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => true,
        _ => a_name.trim().eq_ignore_ascii_case(b_name.trim()),
    }
}

/// Paired phones missing from `offered`, each listed once, by name.
///
/// A phone paired over Classic Bluetooth that Windows offers no audio from is
/// [`UnavailableReason::NoAudioService`]. A phone paired only over Low Energy
/// is [`UnavailableReason::LowEnergyOnly`]; its Low Energy entry is ignored
/// when the same phone is also paired over Classic.
pub fn unavailable_phones(
    paired: &[PairedDevice],
    offered: &[OfferedSource],
) -> Vec<UnavailableSource> {
    let is_offered = |device: &PairedDevice| {
        offered.iter().any(|source| {
            same_device(
                &device.name,
                device.container.as_deref(),
                &source.name,
                source.container.as_deref(),
            )
        })
    };
    let also_classic = |device: &PairedDevice| {
        paired.iter().any(|other| {
            !other.low_energy
                && same_device(
                    &device.name,
                    device.container.as_deref(),
                    &other.name,
                    other.container.as_deref(),
                )
        })
    };

    let mut phones: Vec<UnavailableSource> = Vec::new();
    for device in paired.iter().filter(|device| device.is_phone) {
        if device.name.trim().is_empty() || is_offered(device) {
            continue;
        }
        let reason = if !device.low_energy {
            UnavailableReason::NoAudioService
        } else if also_classic(device) {
            continue;
        } else {
            UnavailableReason::LowEnergyOnly
        };
        if !phones
            .iter()
            .any(|phone| phone.name.eq_ignore_ascii_case(&device.name))
        {
            phones.push(UnavailableSource::new(device.name.trim(), reason));
        }
    }
    phones.sort_by_key(|phone| phone.name.to_lowercase());
    phones
}

/// A GUID as Windows writes it in queries: `{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}`.
pub fn guid_text(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> String {
    let [a, b, c, d, e, f, g, h] = data4;
    format!(
        "{{{data1:08x}-{data2:04x}-{data3:04x}-{a:02x}{b:02x}-{c:02x}{d:02x}{e:02x}{f:02x}{g:02x}{h:02x}}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str, container: Option<&str>, low_energy: bool) -> PairedDevice {
        PairedDevice {
            name: name.into(),
            container: container.map(Into::into),
            low_energy,
            is_phone: true,
        }
    }

    fn offered(name: &str, container: Option<&str>) -> OfferedSource {
        OfferedSource {
            name: name.into(),
            container: container.map(Into::into),
        }
    }

    #[test]
    fn a_phone_windows_offers_is_not_listed() {
        let paired = [device("OPPO A83", Some("{1}"), false)];
        assert!(unavailable_phones(&paired, &[offered("OPPO A83", Some("{1}"))]).is_empty());
        // Matched by name when the containers are unknown.
        assert!(unavailable_phones(&paired, &[offered("oppo a83 ", None)]).is_empty());
    }

    #[test]
    fn a_classic_phone_without_audio_is_listed() {
        let paired = [
            device("OPPO A83", Some("{1}"), false),
            device("Pixel 7", Some("{2}"), false),
        ];
        assert_eq!(
            unavailable_phones(&paired, &[offered("OPPO A83", Some("{1}"))]),
            [UnavailableSource::new(
                "Pixel 7",
                UnavailableReason::NoAudioService
            )]
        );
    }

    #[test]
    fn the_same_container_matches_despite_a_different_name() {
        let paired = [device("Pixel 7", Some("{ABC}"), false)];
        assert!(unavailable_phones(&paired, &[offered("Pixel 7 audio", Some("{abc}"))]).is_empty());
    }

    #[test]
    fn a_low_energy_only_phone_says_so() {
        let paired = [device("Pixel 7", Some("{2}"), true)];
        assert_eq!(
            unavailable_phones(&paired, &[]),
            [UnavailableSource::new(
                "Pixel 7",
                UnavailableReason::LowEnergyOnly
            )]
        );
    }

    #[test]
    fn a_phone_paired_both_ways_is_judged_by_its_classic_pairing() {
        let paired = [
            device("Pixel 7", Some("{2}"), true),
            device("Pixel 7", Some("{2}"), false),
        ];
        assert!(unavailable_phones(&paired, &[offered("Pixel 7", Some("{2}"))]).is_empty());
        assert_eq!(
            unavailable_phones(&paired, &[]),
            [UnavailableSource::new(
                "Pixel 7",
                UnavailableReason::NoAudioService
            )]
        );
    }

    #[test]
    fn other_devices_and_nameless_entries_are_ignored() {
        let mut buds = device("TWS", Some("{3}"), false);
        buds.is_phone = false;
        let nameless = device("  ", None, false);
        assert!(unavailable_phones(&[buds, nameless], &[]).is_empty());
    }

    #[test]
    fn each_phone_is_listed_once_in_name_order() {
        let paired = [
            device("Pixel 7", Some("{2}"), false),
            device("Galaxy", Some("{4}"), false),
            device("pixel 7", Some("{5}"), false),
        ];
        let names: Vec<_> = unavailable_phones(&paired, &[])
            .into_iter()
            .map(|phone| phone.name)
            .collect();
        assert_eq!(names, ["Galaxy", "Pixel 7"]);
    }

    #[test]
    fn guids_are_written_the_windows_way() {
        let text = guid_text(
            0xe0cb_f06c,
            0xcd8b,
            0x4647,
            [0xbb, 0x8a, 0x26, 0x3b, 0x43, 0xf0, 0xf9, 0x74],
        );
        assert_eq!(text, "{e0cbf06c-cd8b-4647-bb8a-263b43f0f974}");
    }
}
