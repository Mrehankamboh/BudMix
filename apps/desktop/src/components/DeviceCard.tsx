import { isEngaged, profileLabel, statusLabel } from "../lib/format";
import type { DeviceView } from "../lib/types";

interface DeviceCardProps {
  device: DeviceView;
  isHotkeyTarget: boolean;
  onToggle: (device: DeviceView) => void;
  onMakeHotkeyTarget: (id: string) => void;
}

/** One paired Bluetooth audio device as a row: state, profiles, connect. */
export function DeviceCard({
  device,
  isHotkeyTarget,
  onToggle,
  onMakeHotkeyTarget,
}: DeviceCardProps) {
  const engaged = isEngaged(device.state);
  const pending = device.state === "connecting" || device.state === "disconnecting";

  return (
    <li className={`device device--${device.state}`} aria-label={device.name} aria-busy={pending}>
      <span className="device__dot" aria-hidden="true" />
      <div className="device__text">
        <p className="device__name">{device.name}</p>
        <p className="device__status">
          <span>{statusLabel(device)}</span>
          <span className="device__profiles" aria-label="Profiles">
            {device.profiles.map((profile) => (
              <span key={profile}>{profileLabel(profile)}</span>
            ))}
          </span>
        </p>
        <label className="device__hotkey">
          <input
            type="radio"
            name="hotkey-target"
            checked={isHotkeyTarget}
            onChange={() => onMakeHotkeyTarget(device.id)}
          />
          Hotkey target
        </label>
        {device.lastError && (
          <p className="device__error" role="alert">
            {device.lastError}
          </p>
        )}
      </div>
      <button
        type="button"
        className={engaged ? "button button--ghost" : "button button--accent"}
        aria-label={`${engaged ? "Disconnect" : "Connect"} ${device.name}`}
        onClick={() => onToggle(device)}
      >
        {engaged ? "Disconnect" : "Connect"}
      </button>
    </li>
  );
}
