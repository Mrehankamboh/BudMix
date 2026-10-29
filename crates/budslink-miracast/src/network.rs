//! Is the connected network Private? The receiver only runs on networks the
//! user trusts (Private or a work domain), never on Public Wi-Fi.

use windows::Win32::Networking::NetworkListManager::{
    INetwork, INetworkListManager, NLM_ENUM_NETWORK_CONNECTED, NLM_NETWORK_CATEGORY_PUBLIC,
    NetworkListManager,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};

/// `Some(true)` when every connected network is Private or Domain,
/// `Some(false)` when one is Public, `None` when nothing is connected or
/// Windows can't tell.
pub fn connected_networks_private() -> Option<bool> {
    // SAFETY: COM initialisation for this thread, then plain COM calls.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok()?;
        let networks = manager.GetNetworks(NLM_ENUM_NETWORK_CONNECTED).ok()?;
        let mut seen = false;
        loop {
            let mut batch: [Option<INetwork>; 1] = [None];
            let mut fetched = 0u32;
            if networks.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(network) = batch[0].take() else {
                break;
            };
            seen = true;
            if network.GetCategory().ok()? == NLM_NETWORK_CATEGORY_PUBLIC {
                return Some(false);
            }
        }
        seen.then_some(true)
    }
}
