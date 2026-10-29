//! Text shown in the tray. Pure functions, unit-tested.

use budslink_core::{BridgeState, ConnectionState, DeviceView, SourceView};

pub fn menu_label(view: &DeviceView) -> String {
    let status = match (view.state, view.reconnect_attempt) {
        (ConnectionState::Connecting, Some(_)) => "reconnecting…",
        (ConnectionState::Disconnected, Some(_)) => "will retry",
        (ConnectionState::Connected, _) => "connected",
        (ConnectionState::Connecting, None) => "connecting…",
        (ConnectionState::Disconnecting, _) => "disconnecting…",
        (ConnectionState::Disconnected, None) => "not connected",
    };
    format!("{} ({status})", view.name)
}

pub fn phone_menu_label(phone: &SourceView) -> String {
    let status = match phone.state {
        BridgeState::Off => return format!("Hear {} in buds", phone.name),
        BridgeState::Connecting => "connecting…",
        BridgeState::Streaming => "playing",
        BridgeState::Waiting => "waiting for phone",
    };
    format!("Hear {} in buds ({status})", phone.name)
}

pub fn tooltip(devices: &[DeviceView], phones: &[SourceView]) -> String {
    let connected: Vec<&str> = devices
        .iter()
        .filter(|view| view.state == ConnectionState::Connected)
        .map(|view| view.name.as_str())
        .collect();
    let mut text = if connected.is_empty() {
        "BudsLink: no buds connected".to_owned()
    } else {
        format!("BudsLink: {} connected", connected.join(", "))
    };
    let streaming: Vec<&str> = phones
        .iter()
        .filter(|phone| phone.state == BridgeState::Streaming)
        .map(|phone| phone.name.as_str())
        .collect();
    if !streaming.is_empty() {
        text.push_str(&format!(" + {} audio", streaming.join(", ")));
    }
    text
}

#[cfg(test)]
mod tests {
    use budslink_core::DeviceId;

    use super::*;

    fn view(name: &str, state: ConnectionState, attempt: Option<u32>) -> DeviceView {
        DeviceView {
            id: DeviceId::new(name),
            name: name.to_owned(),
            profiles: Vec::new(),
            state,
            reconnect_attempt: attempt,
            last_error: None,
            last_connected: false,
        }
    }

    fn phone(state: BridgeState) -> SourceView {
        SourceView {
            id: DeviceId::new("p"),
            name: "Pixel".to_owned(),
            enabled: state != BridgeState::Off,
            state,
            last_error: None,
        }
    }

    #[test]
    fn menu_labels() {
        use ConnectionState::*;
        assert_eq!(
            menu_label(&view("Buds", Connected, None)),
            "Buds (connected)"
        );
        assert_eq!(
            menu_label(&view("Buds", Connecting, None)),
            "Buds (connecting…)"
        );
        assert_eq!(
            menu_label(&view("Buds", Connecting, Some(2))),
            "Buds (reconnecting…)"
        );
        assert_eq!(
            menu_label(&view("Buds", Disconnected, Some(2))),
            "Buds (will retry)"
        );
        assert_eq!(
            menu_label(&view("Buds", Disconnected, None)),
            "Buds (not connected)"
        );
        assert_eq!(
            menu_label(&view("Buds", Disconnecting, None)),
            "Buds (disconnecting…)"
        );
    }

    #[test]
    fn phone_labels() {
        assert_eq!(
            phone_menu_label(&phone(BridgeState::Off)),
            "Hear Pixel in buds"
        );
        assert_eq!(
            phone_menu_label(&phone(BridgeState::Streaming)),
            "Hear Pixel in buds (playing)"
        );
        assert_eq!(
            phone_menu_label(&phone(BridgeState::Waiting)),
            "Hear Pixel in buds (waiting for phone)"
        );
    }

    #[test]
    fn tooltips() {
        use ConnectionState::*;
        assert_eq!(tooltip(&[], &[]), "BudsLink: no buds connected");
        assert_eq!(
            tooltip(
                &[
                    view("A", Connected, None),
                    view("B", Connecting, None),
                    view("C", Connected, None)
                ],
                &[phone(BridgeState::Waiting)]
            ),
            "BudsLink: A, C connected"
        );
        assert_eq!(
            tooltip(
                &[view("A", Connected, None)],
                &[phone(BridgeState::Streaming)]
            ),
            "BudsLink: A connected + Pixel audio"
        );
    }
}
