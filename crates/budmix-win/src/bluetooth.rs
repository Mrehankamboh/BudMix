//! What Windows knows about paired Bluetooth devices: which phones it won't
//! stream audio from, shown in the UI with the reason, and a plain-text report
//! for bug reports. Every query is best effort; a failure is logged or shown
//! in the report and never affects bridge mode itself.

use windows::Devices::Enumeration::{
    DeviceInformation, DeviceInformationCollection, DeviceInformationKind,
};
use windows::Foundation::{IPropertyValue, PropertyType};
use windows::core::{HSTRING, Interface, Result};
use windows_collections::IIterable;

use budmix_core::UnavailableSource;

use crate::paired::{
    self, OfferedSource, PHONE_APPEARANCE_CATEGORY, PHONE_MAJOR_CLASS, PairedDevice,
};

const CLASSIC_PROTOCOL: &str = "{e0cbf06c-cd8b-4647-bb8a-263b43f0f974}";
const LOW_ENERGY_PROTOCOL: &str = "{bb7bb05e-5972-42b5-94fc-76eaa7084d49}";
const AEP_CONTAINER: &str = "System.Devices.Aep.ContainerId";
const AEP_CONNECTED: &str = "System.Devices.Aep.IsConnected";
const COD_MAJOR: &str = "System.Devices.Aep.Bluetooth.Cod.Major";
const LE_CATEGORY: &str = "System.Devices.Aep.Bluetooth.Le.Appearance.Category";
const CONTAINER: &str = "System.Devices.ContainerId";
/// The clause of the phone audio selector that hides disabled entries.
const ENABLED_ONLY: &str =
    " AND System.Devices.InterfaceEnabled:=System.StructuredQueryType.Boolean#True";

/// A paired device with the details the report shows.
struct Paired {
    device: PairedDevice,
    class: Option<u32>,
    connected: Option<bool>,
}

/// One entry of the phone audio selector.
struct Offered {
    source: OfferedSource,
    id: String,
    enabled: bool,
}

/// Paired phones Windows won't stream audio from.
pub fn unavailable_phones(selector: &str) -> Vec<UnavailableSource> {
    let offered: Vec<OfferedSource> = match offered_sources(selector) {
        Ok(entries) => entries.into_iter().map(|entry| entry.source).collect(),
        Err(err) => {
            log::warn!("listing phone audio sources failed: {err}");
            return Vec::new();
        }
    };
    let mut devices = Vec::new();
    for low_energy in [false, true] {
        match paired_devices(low_energy) {
            Ok(paired) => devices.extend(paired.into_iter().map(|paired| paired.device)),
            Err(err) => log::warn!(
                "listing paired {} devices failed: {err}",
                transport(low_energy)
            ),
        }
    }
    paired::unavailable_phones(&devices, &offered)
}

/// What Windows reports about phones and other paired devices, as text.
pub fn report(selector: &str) -> String {
    let mut lines = vec![format!("Phone audio selector: {selector}")];

    // Disabled entries too: they show a phone Windows knows but won't use.
    match offered_sources(&selector.replace(ENABLED_ONLY, "")) {
        Ok(entries) => {
            lines.push(format!("Phone audio sources ({}):", entries.len()));
            for entry in entries {
                lines.push(format!(
                    "  {} · {} · container {} · {}",
                    entry.source.name,
                    if entry.enabled { "enabled" } else { "disabled" },
                    entry.source.container.as_deref().unwrap_or("?"),
                    entry.id
                ));
            }
        }
        Err(err) => lines.push(format!("Phone audio sources: query failed: {err}")),
    }

    for low_energy in [false, true] {
        let transport = transport(low_energy);
        match paired_devices(low_energy) {
            Ok(devices) => {
                lines.push(format!("Paired {transport} devices ({}):", devices.len()));
                for paired in devices {
                    lines.push(format!(
                        "  {} · {} · {} · container {}",
                        paired.device.name,
                        class_label(low_energy, paired.class),
                        match paired.connected {
                            Some(true) => "connected",
                            Some(false) => "not connected",
                            None => "connection unknown",
                        },
                        paired.device.container.as_deref().unwrap_or("?")
                    ));
                    // The device nodes show which Bluetooth services Windows
                    // set up, e.g. {0000110a-…} for a phone's audio.
                    let worth_listing = paired.device.is_phone || paired.class.is_none();
                    if let (true, Some(container)) = (worth_listing, &paired.device.container) {
                        match device_nodes(container) {
                            Ok(nodes) => lines.extend(nodes.into_iter().map(|(id, enabled)| {
                                format!("    {id}{}", if enabled { "" } else { " (disabled)" })
                            })),
                            Err(err) => {
                                lines.push(format!("    device nodes: query failed: {err}"))
                            }
                        }
                    }
                }
            }
            Err(err) => lines.push(format!("Paired {transport} devices: query failed: {err}")),
        }
    }

    let unavailable = unavailable_phones(selector);
    lines.push(format!(
        "Phones shown as unavailable: {}",
        if unavailable.is_empty() {
            "none".to_owned()
        } else {
            unavailable
                .iter()
                .map(|phone| format!("{} ({:?})", phone.name, phone.reason))
                .collect::<Vec<_>>()
                .join(", ")
        }
    ));
    lines.join("\n")
}

fn transport(low_energy: bool) -> &'static str {
    if low_energy {
        "Bluetooth LE"
    } else {
        "Bluetooth Classic"
    }
}

fn class_label(low_energy: bool, class: Option<u32>) -> String {
    match (low_energy, class) {
        (_, None) => "class unknown".to_owned(),
        (false, Some(PHONE_MAJOR_CLASS)) | (true, Some(PHONE_APPEARANCE_CATEGORY)) => {
            "phone".to_owned()
        }
        (false, Some(1)) => "computer".to_owned(),
        (false, Some(4)) => "audio".to_owned(),
        (false, Some(5)) => "peripheral".to_owned(),
        (false, Some(class)) => format!("class {class}"),
        (true, Some(category)) => format!("appearance {category}"),
    }
}

fn find_all(
    aqs: &str,
    properties: &[&str],
    kind: DeviceInformationKind,
) -> Result<DeviceInformationCollection> {
    let properties: Vec<HSTRING> = properties.iter().map(|name| HSTRING::from(*name)).collect();
    DeviceInformation::FindAllAsyncWithKindAqsFilterAndAdditionalProperties(
        &HSTRING::from(aqs),
        &IIterable::from(properties),
        kind,
    )?
    .join()
}

fn paired_devices(low_energy: bool) -> Result<Vec<Paired>> {
    let (protocol, class_property, phone_class) = if low_energy {
        (LOW_ENERGY_PROTOCOL, LE_CATEGORY, PHONE_APPEARANCE_CATEGORY)
    } else {
        (CLASSIC_PROTOCOL, COD_MAJOR, PHONE_MAJOR_CLASS)
    };
    let aqs = format!(
        "System.Devices.Aep.ProtocolId:=\"{protocol}\" AND \
         System.Devices.Aep.IsPaired:=System.StructuredQueryType.Boolean#True"
    );
    let devices = find_all(
        &aqs,
        &[AEP_CONTAINER, AEP_CONNECTED, class_property],
        DeviceInformationKind::AssociationEndpoint,
    )?;
    Ok(devices
        .into_iter()
        .map(|info| {
            let class = number(&info, class_property);
            Paired {
                device: PairedDevice {
                    name: info.Name().map(|name| name.to_string()).unwrap_or_default(),
                    container: guid(&info, AEP_CONTAINER),
                    low_energy,
                    is_phone: class == Some(phone_class),
                },
                class,
                connected: value(&info, AEP_CONNECTED).and_then(|value| value.GetBoolean().ok()),
            }
        })
        .collect())
}

fn offered_sources(aqs: &str) -> Result<Vec<Offered>> {
    let kind = DeviceInformationKind::DeviceInterface;
    let devices = find_all(aqs, &[CONTAINER], kind).or_else(|err| {
        log::debug!("phone audio sources without containers ({err})");
        find_all(aqs, &[], kind)
    })?;
    Ok(devices
        .into_iter()
        .map(|info| Offered {
            source: OfferedSource {
                name: info.Name().map(|name| name.to_string()).unwrap_or_default(),
                container: guid(&info, CONTAINER),
            },
            id: info.Id().map(|id| id.to_string()).unwrap_or_default(),
            enabled: info.IsEnabled().unwrap_or(false),
        })
        .collect())
}

/// Device nodes (instance ID, enabled) in one device container.
fn device_nodes(container: &str) -> Result<Vec<(String, bool)>> {
    let aqs = format!("System.Devices.ContainerId:=\"{container}\"");
    let nodes = find_all(&aqs, &[], DeviceInformationKind::Device)?;
    Ok(nodes
        .into_iter()
        .map(|node| {
            (
                node.Id().map(|id| id.to_string()).unwrap_or_default(),
                node.IsEnabled().unwrap_or(false),
            )
        })
        .collect())
}

fn value(info: &DeviceInformation, name: &str) -> Option<IPropertyValue> {
    info.Properties()
        .ok()?
        .Lookup(&HSTRING::from(name))
        .ok()?
        .cast()
        .ok()
}

/// A numeric property, whatever integer type Windows stores it as.
fn number(info: &DeviceInformation, name: &str) -> Option<u32> {
    let value = value(info, name)?;
    match value.Type().ok()? {
        PropertyType::UInt8 => value.GetUInt8().ok().map(u32::from),
        PropertyType::UInt16 => value.GetUInt16().ok().map(u32::from),
        PropertyType::UInt32 => value.GetUInt32().ok(),
        PropertyType::Int16 => value.GetInt16().ok().and_then(|n| u32::try_from(n).ok()),
        PropertyType::Int32 => value.GetInt32().ok().and_then(|n| u32::try_from(n).ok()),
        _ => None,
    }
}

fn guid(info: &DeviceInformation, name: &str) -> Option<String> {
    let guid = value(info, name)?.GetGuid().ok()?;
    Some(paired::guid_text(
        guid.data1, guid.data2, guid.data3, guid.data4,
    ))
}
