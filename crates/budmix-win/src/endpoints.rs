//! Enumerates Bluetooth render endpoints through the Core Audio API.

use windows::Win32::Devices::FunctionDiscovery::{
    PKEY_Device_ContainerId, PKEY_Device_FriendlyName, PKEY_DeviceInterface_FriendlyName,
};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, DEVICE_STATE_UNPLUGGED, EDataFlow, IConnector,
    IDeviceTopology, IMMDevice, IMMDeviceEnumerator, IPart, eRender,
};
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantClear, PropVariantToGUID, PropVariantToStringAlloc,
};
use windows::Win32::System::Com::{CLSCTX_ALL, STGM_READ};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::core::{GUID, Interface, Result};

use crate::classify::{self, EndpointInfo};
use crate::com::take_pwstr;

/// All Bluetooth render endpoints that are connected (`ACTIVE`) or paired
/// but disconnected (`UNPLUGGED`). Endpoints that fail to read are skipped.
pub(crate) fn bluetooth_endpoints(enumerator: &IMMDeviceEnumerator) -> Result<Vec<EndpointInfo>> {
    let states = DEVICE_STATE(DEVICE_STATE_ACTIVE.0 | DEVICE_STATE_UNPLUGGED.0);
    let endpoints = bluetooth_endpoints_with_ids(enumerator, eRender, states)?;
    Ok(endpoints.into_iter().map(|(_, info)| info).collect())
}

/// Bluetooth endpoints of one data flow and state, with their Core Audio
/// endpoint ids (the ids `IMMDeviceEnumerator::GetDevice` accepts).
pub(crate) fn bluetooth_endpoints_with_ids(
    enumerator: &IMMDeviceEnumerator,
    flow: EDataFlow,
    states: DEVICE_STATE,
) -> Result<Vec<(String, EndpointInfo)>> {
    // SAFETY: COM calls on a live enumerator; results are owned wrappers.
    let collection = unsafe { enumerator.EnumAudioEndpoints(flow, states)? };
    let count = unsafe { collection.GetCount()? };
    let mut endpoints = Vec::new();
    for index in 0..count {
        let endpoint = unsafe { collection.Item(index) }.and_then(|device| {
            let info = read_endpoint(&device)?;
            Ok(info.zip(endpoint_id(&device)))
        });
        match endpoint {
            Ok(Some((info, id))) => endpoints.push((id, info)),
            Ok(None) => {}
            Err(err) => log::debug!("skipping audio endpoint {index}: {err}"),
        }
    }
    Ok(endpoints)
}

fn read_endpoint(device: &IMMDevice) -> Result<Option<EndpointInfo>> {
    let Some(filter_id) = bluetooth_filter_id(device)? else {
        return Ok(None);
    };
    let Some(profile) = classify::bluetooth_profile(&filter_id) else {
        return Ok(None);
    };
    // SAFETY: COM calls on a live device.
    let (state, store) = unsafe { (device.GetState()?, device.OpenPropertyStore(STGM_READ)?) };
    let Some(container) = read_guid(&store, &PKEY_Device_ContainerId) else {
        return Ok(None);
    };
    let name = read_string(&store, &PKEY_DeviceInterface_FriendlyName)
        .or_else(|| read_string(&store, &PKEY_Device_FriendlyName))
        .unwrap_or_default();
    Ok(Some(EndpointInfo {
        container_id: format!("{{{container:?}}}"),
        name,
        active: state == DEVICE_STATE_ACTIVE,
        filter_id,
        profile,
    }))
}

/// Follows the endpoint's connector to the KS filter on the other side and
/// returns its device id if it is a Bluetooth filter.
fn bluetooth_filter_id(device: &IMMDevice) -> Result<Option<String>> {
    // SAFETY: COM calls on live interfaces; strings are freed by `take_pwstr`.
    unsafe {
        let topology: IDeviceTopology = device.Activate(CLSCTX_ALL, None)?;
        for index in 0..topology.GetConnectorCount()? {
            let connector: IConnector = topology.GetConnector(index)?;
            // Fails with E_NOTFOUND when nothing is attached; not an error for us.
            let Ok(other) = connector.GetConnectedTo() else {
                continue;
            };
            let part: IPart = other.cast()?;
            let filter = part.GetTopologyObject()?;
            if let Some(id) = take_pwstr(filter.GetDeviceId()?) {
                if classify::bluetooth_profile(&id).is_some() {
                    return Ok(Some(id));
                }
            }
        }
    }
    Ok(None)
}

fn read_string(store: &IPropertyStore, key: &PROPERTYKEY) -> Option<String> {
    // SAFETY: the PROPVARIANT is cleared after use; the string is freed.
    unsafe {
        let mut value = store.GetValue(key).ok()?;
        let text = PropVariantToStringAlloc(&value)
            .ok()
            .and_then(|pwstr| take_pwstr(pwstr));
        let _ = PropVariantClear(&mut value);
        text.filter(|text| !text.trim().is_empty())
    }
}

fn read_guid(store: &IPropertyStore, key: &PROPERTYKEY) -> Option<GUID> {
    // SAFETY: the PROPVARIANT is cleared after use.
    unsafe {
        let mut value = store.GetValue(key).ok()?;
        let guid = PropVariantToGUID(&value).ok();
        let _ = PropVariantClear(&mut value);
        guid.filter(|guid| *guid != GUID::zeroed())
    }
}

/// The endpoint's Core Audio id.
pub(crate) fn endpoint_id(device: &IMMDevice) -> Option<String> {
    // SAFETY: the returned string is freed by `take_pwstr`.
    unsafe { device.GetId().ok().and_then(|id| take_pwstr(id)) }
}

/// The endpoint's friendly name, e.g. "Headphones (TWS)".
pub(crate) fn endpoint_name(device: &IMMDevice) -> Option<String> {
    // SAFETY: COM call on a live device.
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }.ok()?;
    read_string(&store, &PKEY_Device_FriendlyName)
        .or_else(|| read_string(&store, &PKEY_DeviceInterface_FriendlyName))
}
