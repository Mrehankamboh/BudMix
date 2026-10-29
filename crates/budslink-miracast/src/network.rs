//! Is the laptop on a network the user trusts? The receiver only runs while
//! it is (Private or a work domain), never on Public Wi-Fi alone.

use windows::Win32::Networking::NetworkListManager::{
    INetwork, INetworkListManager, NLM_ENUM_NETWORK_CONNECTED, NLM_NETWORK_CATEGORY_PUBLIC,
    NetworkListManager,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};

use crate::probe::trusted_network;

/// See [`trusted_network`]: `Some(true)` on a Private or Domain network,
/// `Some(false)` on Public networks only, `None` when nothing is connected.
pub fn connected_networks_private() -> Option<bool> {
    // SAFETY: COM initialisation for this thread, then plain COM calls.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok()?;
        let networks = manager.GetNetworks(NLM_ENUM_NETWORK_CONNECTED).ok()?;
        let mut private = Vec::new();
        loop {
            let mut batch: [Option<INetwork>; 1] = [None];
            let mut fetched = 0u32;
            if networks.Next(&mut batch, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }
            let Some(network) = batch[0].take() else {
                break;
            };
            if let Ok(category) = network.GetCategory() {
                private.push(category != NLM_NETWORK_CATEGORY_PUBLIC);
            }
        }
        trusted_network(&private)
    }
}
