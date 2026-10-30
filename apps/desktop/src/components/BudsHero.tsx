import { statusLabel } from "../lib/format";
import { playingCount, type SourceRow } from "../lib/sources";
import type { AudioView, DeviceView } from "../lib/types";
import { BudsArt, type BudsArtState } from "./BudsArt";

interface BudsHeroProps {
  device: DeviceView | null;
  audio: AudioView | null;
  rows: SourceRow[];
  demo: boolean;
  tone: "idle" | "playing";
  onTestBeep: () => void;
}

function artState(device: DeviceView | null): BudsArtState {
  if (device?.state === "connected") return "connected";
  if (device?.state === "connecting" || device?.state === "disconnecting") return "busy";
  return "off";
}

/** The top of the settings window: the buds, what they play, and a test beep. */
export function BudsHero({ device, audio, rows, demo, tone, onTestBeep }: BudsHeroProps) {
  const playing = rows.filter((row) => row.source !== "master" && row.status === "playing");
  const count = playingCount(rows);

  return (
    <header className="hero-card">
      <div className="hero-card__art">
        <BudsArt state={artState(device)} />
      </div>
      <div className="hero-card__text">
        <p className="hero-card__brand">
          <span>BudMix</span>
          {demo && <span className="badge">Demo mode: simulated devices</span>}
        </p>
        <h1 className="hero-card__device">{device?.name ?? "No buds yet"}</h1>
        <p className="hero-card__status">
          {device ? statusLabel(device) : "Pair your buds in Windows Settings → Bluetooth & devices"}
          {audio?.callMode && <span className="hero-card__warn"> · call mode (HFP)</span>}
        </p>
        <p className="hero-card__sources">
          <strong>{count}</strong> {count === 1 ? "source" : "sources"} in your buds
          {playing.length > 0 && (
            <span className="hero-card__chips">
              {playing.map((row) => (
                <span key={row.source} className={`chip chip--${row.source}`}>
                  {row.source === "laptop" ? "Laptop" : row.source === "phoneWifi" ? "Phone · Wi-Fi" : "Phone · Bluetooth"}
                </span>
              ))}
            </span>
          )}
        </p>
        <button
          type="button"
          className="button button--ghost"
          disabled={tone === "playing" || device?.state !== "connected"}
          onClick={onTestBeep}
        >
          {tone === "playing" ? "Playing…" : "Test beep"}
        </button>
      </div>
    </header>
  );
}
