import { useState, type FormEvent } from "react";

import type { PhoneMode, Settings } from "../lib/types";

interface SettingsPanelProps {
  settings: Settings;
  warnings: string[];
  onChange: (next: Settings) => void;
}

export function SettingsPanel({ settings, warnings, onChange }: SettingsPanelProps) {
  const [hotkey, setHotkey] = useState(settings.toggleHotkey ?? "");
  // Follow a hotkey changed elsewhere (e.g. saved, or reset) during render, not
  // in an effect, so a late effect can never overwrite what is being typed.
  const [syncedHotkey, setSyncedHotkey] = useState(settings.toggleHotkey);
  if (settings.toggleHotkey !== syncedHotkey) {
    setSyncedHotkey(settings.toggleHotkey);
    setHotkey(settings.toggleHotkey ?? "");
  }

  const saveHotkey = (event: FormEvent) => {
    event.preventDefault();
    const trimmed = hotkey.trim();
    onChange({ ...settings, toggleHotkey: trimmed === "" ? null : trimmed });
  };

  return (
    <section className="panel" aria-labelledby="settings-title">
      <h2 id="settings-title">Settings</h2>

      <label className="switch">
        <input
          type="checkbox"
          checked={settings.autoReconnect}
          onChange={(event) => onChange({ ...settings, autoReconnect: event.target.checked })}
        />
        <span>
          Auto-reconnect
          <small>Retry when buds drop unexpectedly. A manual disconnect never retries.</small>
        </span>
      </label>

      <label className="switch">
        <input
          type="checkbox"
          checked={settings.routeLaptopAudio}
          onChange={(event) => onChange({ ...settings, routeLaptopAudio: event.target.checked })}
        />
        <span>
          Play laptop audio in my buds
          <small>
            Makes the connected buds the default output, and gives your previous output back when
            they disconnect or you turn this off.
          </small>
        </span>
      </label>

      <label className="field">
        <span>
          Phone connection
          <small>Auto uses Wi-Fi while the phone casts and Bluetooth otherwise, never both at once.</small>
        </span>
        <select
          value={settings.phoneMode}
          onChange={(event) => onChange({ ...settings, phoneMode: event.target.value as PhoneMode })}
        >
          <option value="auto">Auto (Wi-Fi when casting, else Bluetooth)</option>
          <option value="wifiOnly">Wi-Fi only (Cast)</option>
          <option value="bluetoothOnly">Bluetooth only</option>
        </select>
      </label>

      <label className="switch">
        <input
          type="checkbox"
          checked={settings.showCastVideo}
          onChange={(event) => onChange({ ...settings, showCastVideo: event.target.checked })}
        />
        <span>
          Show the phone&apos;s screen while it casts
          <small>A small window you can hide anytime. The sound keeps playing either way.</small>
        </span>
      </label>

      <label className="switch">
        <input
          type="checkbox"
          checked={settings.startWithWindows}
          onChange={(event) => onChange({ ...settings, startWithWindows: event.target.checked })}
        />
        <span>
          Start with Windows
          <small>Opens in the tray, ready for the hotkey.</small>
        </span>
      </label>

      <form className="hotkey" onSubmit={saveHotkey}>
        <label htmlFor="hotkey-input">Toggle hotkey</label>
        <div className="hotkey__row">
          <input
            id="hotkey-input"
            value={hotkey}
            placeholder="e.g. Ctrl+Alt+B (empty = off)"
            onChange={(event) => setHotkey(event.target.value)}
            spellCheck={false}
          />
          <button type="submit" className="button">
            Save
          </button>
        </div>
      </form>

      {warnings.length > 0 && (
        <ul className="warnings" role="alert">
          {warnings.map((warning) => (
            <li key={warning}>{warning}</li>
          ))}
        </ul>
      )}
    </section>
  );
}
