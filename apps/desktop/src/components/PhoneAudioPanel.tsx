import { UNAVAILABLE_HINT, bridgeStatusLabel, unavailableLabel } from "../lib/format";
import type { BridgeState, SourceView, UnavailablePhone } from "../lib/types";

interface PhoneAudioPanelProps {
  phones: SourceView[] | null;
  /** Paired phones Windows won't stream from. */
  unavailable?: UnavailablePhone[];
  onToggle: (phone: SourceView, enabled: boolean) => void;
}

// Reuses the device row look: green when playing, amber while working on it.
const ROW_STATE: Record<BridgeState, string> = {
  off: "disconnected",
  connecting: "connecting",
  waiting: "connecting",
  streaming: "connected",
  paused: "disconnected",
};

export function PhoneAudioPanel({ phones, unavailable = [], onToggle }: PhoneAudioPanelProps) {
  return (
    <section className="section" aria-labelledby="phone-title">
      <header className="section__head">
        <div>
          <h2 id="phone-title">Phone over Bluetooth</h2>
          <p className="section__lead">
            The laptop acts as a speaker for your phone, so its sound reaches the buds without
            switching. Nothing to tap on the phone.
          </p>
        </div>
      </header>

      {phones === null ? (
        <p className="muted">Looking for paired phones…</p>
      ) : phones.length + unavailable.length === 0 ? (
        <p className="muted">
          No phone is paired with this laptop yet. Pair it in Windows Settings → Bluetooth &amp;
          devices → Add device, and it shows up here.
        </p>
      ) : (
        <ul className="rows-list">
          {phones.map((phone) => (
            <li
              key={phone.id}
              className={`device device--${ROW_STATE[phone.state]}`}
              aria-label={phone.name}
              aria-busy={phone.state === "connecting"}
            >
              <span className="device__dot" aria-hidden="true" />
              <div className="device__text">
                <p className="device__name">{phone.name}</p>
                <p className="device__status">{bridgeStatusLabel(phone.state)}</p>
                {phone.lastError && (
                  <p className="device__error" role="alert">
                    {phone.lastError}
                  </p>
                )}
              </div>
              <input
                type="checkbox"
                role="switch"
                className="toggle"
                aria-label={`Hear ${phone.name} in my buds`}
                checked={phone.enabled}
                onChange={(event) => onToggle(phone, event.target.checked)}
              />
            </li>
          ))}
          {unavailable.map((phone) => (
            <li
              key={`unavailable:${phone.name}`}
              className="device device--unavailable"
              aria-label={phone.name}
            >
              <span className="device__dot" aria-hidden="true" />
              <div className="device__text">
                <p className="device__name">{phone.name}</p>
                <p className="device__status">{unavailableLabel(phone.reason)}</p>
              </div>
            </li>
          ))}
        </ul>
      )}
      {unavailable.length > 0 && <p className="muted small">{UNAVAILABLE_HINT}</p>}

      <p className="footnote">
        Windows shows a paired phone as “Not connected” until you switch it on here. BudMix then
        connects it and keeps it connected. For calls, Windows Phone Link brings them to the buds
        too.
      </p>
    </section>
  );
}
