import { Bluetooth, Headphones, Laptop, Volume2, VolumeX, Wifi } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { STATUS_LABEL, type SourceRow } from "../lib/sources";
import type { Level, VolumeSource, Volumes } from "../lib/types";

interface SliderProps {
  label: string;
  level: Level;
  onChange: (level: Level) => void;
}

/** Volume slider and mute button. Dragging is smooth; the app hears about it after a pause. */
function VolumeControl({ label, level, onChange }: SliderProps) {
  const [percent, setPercent] = useState(level.percent);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Follow changes made elsewhere (another window, the tray).
  const [synced, setSynced] = useState(level.percent);
  if (level.percent !== synced) {
    setSynced(level.percent);
    setPercent(level.percent);
  }
  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  const move = (value: number) => {
    setPercent(value);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => onChange({ ...level, percent: value }), 200);
  };

  return (
    <span className="volume">
      <button
        type="button"
        className="icon-button"
        aria-label={`${level.muted ? "Unmute" : "Mute"} ${label}`}
        aria-pressed={level.muted}
        onClick={() => onChange({ ...level, muted: !level.muted })}
      >
        {level.muted ? <VolumeX size={14} /> : <Volume2 size={14} />}
      </button>
      <input
        type="range"
        min={0}
        max={100}
        value={percent}
        aria-label={`${label} volume`}
        onChange={(event) => move(Number(event.target.value))}
      />
      <span className="volume__value">{level.muted ? "Muted" : `${percent}%`}</span>
    </span>
  );
}

interface SourcesPanelProps {
  rows: SourceRow[];
  volumes: Volumes;
  onVolume: (source: VolumeSource, level: Level) => void;
  /** Disconnects the casting phone; shown only while one casts. */
  onDisconnectCast?: () => void;
  allowed: string[];
  onForget: (name: string) => void;
  /** Inside the dock: no heading, no allowlist. */
  compact?: boolean;
}

/** Everything that plays in the buds, each with its own volume. */
export function SourcesPanel({
  rows,
  volumes,
  onVolume,
  onDisconnectCast,
  allowed,
  onForget,
  compact = false,
}: SourcesPanelProps) {
  const list = (
    <ul className={`sources${compact ? " sources--compact" : ""}`}>
      {rows.map((row) => {
        const Icon = SOURCE_ICON[row.source];
        return (
          <li key={row.source} className={`source source--${row.status}`} aria-label={row.title}>
            <span className="source__icon" aria-hidden="true">
              <Icon size={16} />
            </span>
            <span className="source__text">
              <span className="source__title">{row.title}</span>
              <small>
                <span className="source__status">{STATUS_LABEL[row.status]}</span>
                {row.detail ? ` · ${row.detail}` : ""}
              </small>
            </span>
            {row.source === "phoneWifi" &&
              onDisconnectCast &&
              (row.status === "playing" || row.status === "connecting") && (
                <button type="button" className="link-button" onClick={onDisconnectCast}>
                  Disconnect
                </button>
              )}
            <VolumeControl
              label={row.source === "master" ? "Buds" : row.title}
              level={volumes[row.source]}
              onChange={(level) => onVolume(row.source, level)}
            />
          </li>
        );
      })}
    </ul>
  );

  if (compact) return list;

  return (
    <section className="section" aria-labelledby="sources-title">
      <header className="section__head">
        <div>
          <h2 id="sources-title">Sources in your buds</h2>
          <p className="section__lead">Everything that plays in your buds, each with its own volume.</p>
        </div>
      </header>
      {list}
      <div className="allowed-block">
        <h3 className="subhead">Phones allowed to cast</h3>
        {allowed.length === 0 ? (
          <p className="muted small">None yet. A new phone is asked about the first time it casts.</p>
        ) : (
          <ul className="chips">
            {allowed.map((name) => (
              <li key={name} className="chip chip--removable" aria-label={name}>
                <span>{name}</span>
                <button type="button" className="chip__action" onClick={() => onForget(name)}>
                  Forget
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

const SOURCE_ICON: Record<VolumeSource, typeof Headphones> = {
  master: Headphones,
  laptop: Laptop,
  phoneWifi: Wifi,
  phoneBluetooth: Bluetooth,
};
