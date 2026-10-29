import { UNAVAILABLE_HINT, bridgeStatusLabel, unavailableLabel } from "../lib/format";
import type { BridgeState, SourceView, UnavailablePhone } from "../lib/types";

interface PhoneAudioPanelProps {
  phones: SourceView[] | null;
  /** Paired phones Windows won't stream from. */
  unavailable?: UnavailablePhone[];
  onToggle: (phone: SourceView, enabled: boolean) => void;
}

// Reuses the device card look: green when playing, amber while working on it.
const CARD_STATE: Record<BridgeState, string> = {
  off: "disconnected",
  connecting: "connecting",
  waiting: "connecting",
  streaming: "connected",
};

export function PhoneAudioPanel({ phones, unavailable = [], onToggle }: PhoneAudioPanelProps) {
  return (
    <section className="panel" aria-labelledby="phone-title">
      <h2 id="phone-title">Phone audio</h2>
      <p className="muted">
        Hear your phone through the laptop. The buds stay connected to the laptop and play both, so
        there is nothing to switch.
      </p>
      <p className="muted small">
        Windows shows a paired phone as “Not connected” until you switch it on here. BudsLink then
        connects it and keeps it connected.
      </p>

      {phones === null ? (
        <p className="muted">Looking for paired phones…</p>
      ) : phones.length + unavailable.length === 0 ? (
        <p className="muted">
          No phone is paired with this laptop yet. Pair it in Windows Settings → Bluetooth &amp;
          devices → Add device, and it shows up here.
        </p>
      ) : (
        <ul className="devices">
          {phones.map((phone) => (
            <li
              key={phone.id}
              className={`device device--${CARD_STATE[phone.state]}`}
              aria-label={phone.name}
              aria-busy={phone.state === "connecting"}
            >
              <div className="device__main">
                <span className="device__dot" aria-hidden="true" />
                <div className="device__text">
                  <p className="device__name">{phone.name}</p>
                  <p className="device__status">{bridgeStatusLabel(phone.state)}</p>
                </div>
              </div>
              {phone.lastError && (
                <p className="device__error" role="alert">
                  {phone.lastError}
                </p>
              )}
              <label className="switch">
                <input
                  type="checkbox"
                  role="switch"
                  checked={phone.enabled}
                  onChange={(event) => onToggle(phone, event.target.checked)}
                />
                <span>Hear {phone.name} in my buds</span>
              </label>
            </li>
          ))}
          {unavailable.map((phone) => (
            <li
              key={`unavailable:${phone.name}`}
              className="device device--unavailable"
              aria-label={phone.name}
            >
              <div className="device__main">
                <span className="device__dot" aria-hidden="true" />
                <div className="device__text">
                  <p className="device__name">{phone.name}</p>
                  <p className="device__status">{unavailableLabel(phone.reason)}</p>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
      {unavailable.length > 0 && <p className="muted small">{UNAVAILABLE_HINT}</p>}

      <p className="muted small">
        Phone calls: Windows Phone Link can take your calls on the laptop, so they reach the buds
        too.
      </p>
    </section>
  );
}
