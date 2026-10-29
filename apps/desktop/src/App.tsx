import { DeviceCard } from "./components/DeviceCard";
import { DiagnosticsPanel } from "./components/DiagnosticsPanel";
import { PhoneAudioPanel } from "./components/PhoneAudioPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { useBudsLink } from "./lib/useBudsLink";

/** The full window: every device, every phone and all settings. */
export function App() {
  const {
    devices,
    phones,
    unavailablePhones,
    settings,
    info,
    error,
    saveSettings,
    toggleDevice: toggle,
    togglePhone,
    makeHotkeyTarget,
  } = useBudsLink();

  return (
    <div className="app">
      <header className="app__header">
        <h1>BudsLink</h1>
        {info?.backend === "demo" && <span className="badge">Demo mode: simulated devices</span>}
      </header>

      <main className="app__main">
        <section className="panel" aria-labelledby="devices-title">
          <h2 id="devices-title">Bluetooth audio</h2>
          {error && (
            <p className="banner banner--error" role="alert">
              {error}
            </p>
          )}
          {devices === null ? (
            <p className="muted">Looking for paired devices…</p>
          ) : devices.length === 0 ? (
            <p className="muted">
              No paired Bluetooth audio devices. Pair your buds in Windows Settings → Bluetooth &amp;
              devices, then they show up here.
            </p>
          ) : (
            <ul className="devices">
              {devices.map((device) => (
                <DeviceCard
                  key={device.id}
                  device={device}
                  isHotkeyTarget={settings?.preferredDevice === device.id}
                  onToggle={toggle}
                  onMakeHotkeyTarget={makeHotkeyTarget}
                />
              ))}
            </ul>
          )}
        </section>

        <PhoneAudioPanel phones={phones} unavailable={unavailablePhones} onToggle={togglePhone} />

        {settings && (
          <SettingsPanel
            settings={settings}
            warnings={info?.warnings ?? []}
            onChange={(next) => void saveSettings(next)}
          />
        )}

        <DiagnosticsPanel />
      </main>

      <footer className="app__footer muted">v{info?.version ?? "…"}</footer>
    </div>
  );
}
