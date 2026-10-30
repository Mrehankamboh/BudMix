import { BudsHero } from "./components/BudsHero";
import { ConnectPhoneGuide } from "./components/ConnectPhoneGuide";
import { DeviceCard } from "./components/DeviceCard";
import { DiagnosticsPanel } from "./components/DiagnosticsPanel";
import { PhoneAudioPanel } from "./components/PhoneAudioPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { SetupPanel } from "./components/SetupPanel";
import { SourcesPanel } from "./components/SourcesPanel";
import { api } from "./lib/api";
import { focusDevice } from "./lib/focus";
import { sourceRows } from "./lib/sources";
import { useBudsLink } from "./lib/useBudsLink";

/** The full window: the buds, every source and phone, and all settings. */
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
    audio,
    cast,
    tone,
    playTestTone,
    rejectPhone,
    forgetPhone,
    setVolume,
  } = useBudsLink();
  const device = focusDevice(devices ?? [], settings?.preferredDevice ?? null);
  const rows = sourceRows(device, audio, cast, phones ?? []);

  return (
    <div className="app">
      <BudsHero
        device={device}
        audio={audio}
        rows={rows}
        demo={info?.backend === "demo"}
        tone={tone}
        onTestBeep={() => void playTestTone()}
      />

      <main className="app__main">
        {settings && (
          <SetupPanel
            done={settings.setupDone}
            onDone={() => void saveSettings({ ...settings, setupDone: true })}
            onPlayTone={() => api.playTestTone()}
          />
        )}

        {settings && (
          <SourcesPanel
            rows={rows}
            volumes={settings.volumes}
            onVolume={setVolume}
            onDisconnectCast={rejectPhone}
            allowed={settings.allowedPhones}
            onForget={forgetPhone}
          />
        )}

        <section className="section" aria-labelledby="devices-title">
          <header className="section__head">
            <div>
              <h2 id="devices-title">Bluetooth audio</h2>
              <p className="section__lead">Buds and headphones paired with this laptop.</p>
            </div>
          </header>
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
            <ul className="rows-list">
              {devices.map((item) => (
                <DeviceCard
                  key={item.id}
                  device={item}
                  isHotkeyTarget={settings?.preferredDevice === item.id}
                  onToggle={toggle}
                  onMakeHotkeyTarget={makeHotkeyTarget}
                />
              ))}
            </ul>
          )}
        </section>

        <PhoneAudioPanel phones={phones} unavailable={unavailablePhones} onToggle={togglePhone} />

        <ConnectPhoneGuide />

        {settings && (
          <SettingsPanel
            settings={settings}
            warnings={info?.warnings ?? []}
            onChange={(next) => void saveSettings(next)}
          />
        )}

        <DiagnosticsPanel />
      </main>

      <footer className="app__footer muted">BudsLink v{info?.version ?? "…"} · based on BudsBridge</footer>
    </div>
  );
}
