import { Volume2, VolumeX } from "lucide-react";
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
    <ul className="sources">
      {rows.map((row) => (
        <li key={row.source} className={`source source--${row.status}`} aria-label={row.title}>
          <span className="source__head">
            <span className="row__dot" aria-hidden="true" />
            <span className="source__text">
              <span className="source__title">{row.title}</span>
              <small>
                {STATUS_LABEL[row.status]}
                {row.detail ? ` · ${row.detail}` : ""}
              </small>
            </span>
            {row.source === "phoneWifi" && onDisconnectCast && (row.status === "playing" || row.status === "connecting") && (
              <button type="button" className="pill" onClick={onDisconnectCast}>
                Disconnect
              </button>
            )}
          </span>
          <VolumeControl
            label={row.source === "master" ? "Buds" : row.title}
            level={volumes[row.source]}
            onChange={(level) => onVolume(row.source, level)}
          />
        </li>
      ))}
    </ul>
  );

  if (compact) return list;

  return (
    <section className="panel" aria-labelledby="sources-title">
      <h2 id="sources-title">Sources in your buds</h2>
      {list}
      <h3 className="subhead">Phones allowed to cast</h3>
      {allowed.length === 0 ? (
        <p className="muted">None yet. A new phone is asked about the first time it casts.</p>
      ) : (
        <ul className="allowed">
          {allowed.map((name) => (
            <li key={name} aria-label={name}>
              <span>{name}</span>
              <button type="button" className="pill" onClick={() => onForget(name)}>
                Forget
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
