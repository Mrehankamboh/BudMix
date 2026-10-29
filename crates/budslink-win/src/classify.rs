//! Platform-independent helpers for turning Core Audio endpoint data into
//! BudsLink devices. Kept separate from the COM code so it is tested on
//! every OS.

use budslink_core::{AudioDevice, AudioProfile, DeviceId};

/// Bluetooth profile served by a kernel-streaming filter, judged by its
/// device id, e.g. `{2}.\\?\bthenum#{0000110b-0000-1000-8000-00805f9b34fb}_...`.
///
/// Returns `None` for filters that do not belong to a Bluetooth device.
pub fn bluetooth_profile(filter_id: &str) -> Option<AudioProfile> {
    let lower = filter_id.to_ascii_lowercase();
    let (_, path) = lower.split_once(r"\\?\")?;
    if path.starts_with("bthhfenum") {
        Some(AudioProfile::HandsFree)
    } else if path.starts_with("bthenum") {
        Some(AudioProfile::Stereo)
    } else if path.starts_with("bthle") {
        Some(AudioProfile::LeAudio)
    } else if path.starts_with("bth") {
        Some(AudioProfile::Unknown)
    } else {
        None
    }
}

/// One Bluetooth render endpoint, as read from Core Audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointInfo {
    /// Device container GUID shared by all endpoints of one physical device.
    pub container_id: String,
    pub name: String,
    pub active: bool,
    /// KS filter that accepts `KSPROPSETID_BtAudio` requests.
    pub filter_id: String,
    pub profile: AudioProfile,
}

/// A physical device and the KS filters that control it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceGroup {
    pub device: AudioDevice,
    pub filter_ids: Vec<String>,
}

/// The device's own name from an endpoint's interface name. Windows calls the
/// hands-free interface "<device> Hands-Free" (older builds: "Hands-Free AG
/// Audio") and some stereo ones "<device> Stereo"; users know it as "<device>".
pub fn display_name(raw: &str) -> String {
    const SUFFIXES: [&str; 4] = [
        " Hands-Free AG Audio",
        " Hands-Free AG",
        " Hands-Free",
        " Stereo",
    ];
    let name = raw.trim();
    for suffix in SUFFIXES {
        let Some(cut) = name.len().checked_sub(suffix.len()) else {
            continue;
        };
        if cut > 0 && name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(suffix) {
            return name[..cut].trim_end().to_owned();
        }
    }
    name.to_owned()
}

/// Groups endpoints by container, so "Headphones" (A2DP) and "Headset"
/// (HFP) of the same buds become one device.
pub fn group_endpoints(endpoints: impl IntoIterator<Item = EndpointInfo>) -> Vec<DeviceGroup> {
    let mut groups: Vec<DeviceGroup> = Vec::new();
    for endpoint in endpoints {
        let index = match groups
            .iter()
            .position(|group| group.device.id.as_str() == endpoint.container_id)
        {
            Some(index) => index,
            None => {
                groups.push(DeviceGroup {
                    device: AudioDevice {
                        id: DeviceId::new(endpoint.container_id.clone()),
                        name: String::new(),
                        profiles: Vec::new(),
                        connected: false,
                    },
                    filter_ids: Vec::new(),
                });
                groups.len() - 1
            }
        };
        let group = &mut groups[index];
        // The stereo endpoint carries the plainest name, so it wins.
        let name = display_name(&endpoint.name);
        if !name.is_empty()
            && (group.device.name.is_empty() || endpoint.profile == AudioProfile::Stereo)
        {
            group.device.name = name;
        }
        group.device.connected |= endpoint.active;
        if !group.device.profiles.contains(&endpoint.profile) {
            group.device.profiles.push(endpoint.profile);
            group.device.profiles.sort();
        }
        if !group.filter_ids.contains(&endpoint.filter_id) {
            group.filter_ids.push(endpoint.filter_id);
        }
    }
    for group in &mut groups {
        if group.device.name.is_empty() {
            group.device.name = "Bluetooth audio device".to_owned();
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    const A2DP: &str = r"{2}.\\?\bthenum#{0000110b-0000-1000-8000-00805f9b34fb}_vid&0002054c_pid&0d58#8&2a5c3e1&0&a0c9a0123456_c00000000#{6994ad04-93ef-11d0-a3cc-00a0c9223196}\sbc";
    const HFP: &str = r"{2}.\\?\BTHHFENUM#BthHFPAudio#8&1b2c3d4&0&97#{6994ad04-93ef-11d0-a3cc-00a0c9223196}\BthHfpAudio";

    fn endpoint(container: &str, name: &str, active: bool, filter: &str) -> EndpointInfo {
        EndpointInfo {
            container_id: container.into(),
            name: name.into(),
            active,
            filter_id: filter.into(),
            profile: bluetooth_profile(filter).unwrap(),
        }
    }

    #[test]
    fn classifies_bluetooth_filters() {
        assert_eq!(bluetooth_profile(A2DP), Some(AudioProfile::Stereo));
        assert_eq!(bluetooth_profile(HFP), Some(AudioProfile::HandsFree));
        assert_eq!(
            bluetooth_profile(r"{2}.\\?\bthledevice#abc"),
            Some(AudioProfile::LeAudio)
        );
        assert_eq!(
            bluetooth_profile(r"{2}.\\?\bthfoo#abc"),
            Some(AudioProfile::Unknown)
        );
    }

    #[test]
    fn ignores_non_bluetooth_filters() {
        assert_eq!(
            bluetooth_profile(r"{2}.\\?\hdaudio#func_01&ven_10ec&dev_0257"),
            None
        );
        assert_eq!(bluetooth_profile(r"{2}.\\?\usb#vid_046d&pid_0a87"), None);
        assert_eq!(bluetooth_profile("bthenum-without-path-prefix"), None);
        assert_eq!(bluetooth_profile(""), None);
    }

    #[test]
    fn groups_endpoints_of_the_same_container() {
        let groups = group_endpoints([
            endpoint("{B}", "Buds", false, A2DP),
            endpoint("{B}", "Buds", true, HFP),
            endpoint("{S}", "Speaker", false, A2DP),
        ]);
        assert_eq!(groups.len(), 2);

        let buds = &groups[0];
        assert_eq!(buds.device.id.as_str(), "{B}");
        assert_eq!(buds.device.name, "Buds");
        assert!(buds.device.connected, "any active endpoint means connected");
        assert_eq!(
            buds.device.profiles,
            vec![AudioProfile::Stereo, AudioProfile::HandsFree]
        );
        assert_eq!(buds.filter_ids, vec![A2DP.to_owned(), HFP.to_owned()]);

        assert!(!groups[1].device.connected);
    }

    #[test]
    fn display_names_drop_profile_suffixes() {
        assert_eq!(display_name("IGNITE Hands-Free"), "IGNITE");
        assert_eq!(display_name("Wave Pro Hands-Free AG Audio"), "Wave Pro");
        assert_eq!(display_name("Buds hands-free ag"), "Buds");
        assert_eq!(display_name("Speaker Stereo"), "Speaker");
        assert_eq!(display_name("  TWS  "), "TWS");
        // A name that is only the suffix stays as it is.
        assert_eq!(display_name("Hands-Free"), "Hands-Free");
        assert_eq!(display_name("Écouteurs"), "Écouteurs");
    }

    #[test]
    fn stereo_endpoint_names_the_device() {
        let groups = group_endpoints([
            endpoint("{W}", "Wave Pro Hands-Free", false, HFP),
            endpoint("{W}", "Wave Pro", false, A2DP),
        ]);
        assert_eq!(groups[0].device.name, "Wave Pro");

        // Hands-free only (the stereo endpoint is absent while disconnected).
        let groups = group_endpoints([endpoint("{I}", "IGNITE Hands-Free", false, HFP)]);
        assert_eq!(groups[0].device.name, "IGNITE");
    }

    #[test]
    fn duplicate_filters_are_listed_once_and_names_fall_back() {
        let groups = group_endpoints([
            endpoint("{B}", "", false, A2DP),
            endpoint("{B}", "", false, A2DP),
        ]);
        assert_eq!(groups[0].filter_ids.len(), 1);
        assert_eq!(groups[0].device.name, "Bluetooth audio device");
    }
}
