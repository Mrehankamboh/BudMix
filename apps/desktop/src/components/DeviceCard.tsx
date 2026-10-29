import { isEngaged, profileLabel, statusLabel } from "../lib/format";
import type { DeviceView } from "../lib/types";

interface DeviceCardProps {
  device: DeviceView;
  isHotkeyTarget: boolean;
  onToggle: (device: DeviceView) => void;
  onMakeHotkeyTarget: (id: string) => void;
}

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
      <div className="device__main">
        <span className="device__dot" aria-hidden="true" />
        <div className="device__text">
          <p className="device__name">{device.name}</p>
          <p className="device__status">{statusLabel(device)}</p>
          <ul className="device__profiles" aria-label="Profiles">
            {device.profiles.map((profile) => (
              <li key={profile}>{profileLabel(profile)}</li>
            ))}
          </ul>
        </div>
      </div>

      {device.lastError && (
        <p className="device__error" role="alert">
          {device.lastError}
        </p>
      )}

      <div className="device__actions">
        <button
          type="button"
          className={engaged ? "button" : "button button--accent"}
          aria-label={`${engaged ? "Disconnect" : "Connect"} ${device.name}`}
          onClick={() => onToggle(device)}
        >
          {engaged ? "Disconnect" : "Connect"}
        </button>
        <label className="device__hotkey">
          <input
            type="radio"
            name="hotkey-target"
            checked={isHotkeyTarget}
            onChange={() => onMakeHotkeyTarget(device.id)}
          />
          Hotkey target
        </label>
      </div>
    </li>
  );
}
