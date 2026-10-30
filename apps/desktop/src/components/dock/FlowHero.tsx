import { useId } from "react";

import { bridgeStatusLabel, shortName, statusLabel } from "../../lib/format";
import type { DeviceView, SourceView } from "../../lib/types";

interface FlowHeroProps {
  device: DeviceView | null;
  /** Every paired phone; the ones switched on are drawn, up to two. */
  phones: SourceView[];
}

const PHONE_STATE: Record<SourceView["state"], string> = {
  off: "Off",
  connecting: "Connecting",
  waiting: "Waiting",
  streaming: "Playing",
  paused: "Paused",
};

const LAPTOP = { left: 126, right: 234, mid: 49 };
const BUDS = { x: 310, y: 47 };
const MAX_PHONES_DRAWN = 2;

/** Phones to draw: the ones switched on, playing first, else the first phone. */
function shownPhones(phones: SourceView[]): SourceView[] {
  const on = phones.filter((phone) => phone.enabled);
  const order = (phone: SourceView) => (phone.state === "streaming" ? 0 : 1);
  return on.length > 0 ? [...on].sort((a, b) => order(a) - order(b)) : phones.slice(0, 1);
}

interface Slot {
  x: number;
  y: number;
  w: number;
  h: number;
  tilt: number;
}

const ONE_PHONE: Slot = { x: 52, y: 49, w: 34, h: 62, tilt: 0 };
const TWO_PHONES: [Slot, Slot] = [
  { x: 38, y: 54, w: 30, h: 55, tilt: -7 },
  { x: 68, y: 45, w: 30, h: 55, tilt: 6 },
];

/** Where each drawn phone sits: one upright, or two fanned out. */
function placePhones(shown: SourceView[]): { phone: SourceView | null; slot: Slot }[] {
  const [first, second] = shown;
  if (first && second) {
    return [
      { phone: first, slot: TWO_PHONES[0] },
      { phone: second, slot: TWO_PHONES[1] },
    ];
  }
  return [{ phone: first ?? null, slot: ONE_PHONE }];
}

/** A wave as smooth quadratic curves, long enough to slide one wavelength. */
function wave(x: number, width: number, y: number, amplitude: number, wavelength: number): string {
  const half = wavelength / 2;
  let d = `M${x} ${y} q${wavelength / 4} ${-amplitude} ${half} 0`;
  for (let covered = half; covered < width + wavelength; covered += half) d += ` t${half} 0`;
  return d;
}

/** A link between two points, bowed like a cable. */
function link(x1: number, y1: number, x2: number, y2: number): string {
  const pull = (x2 - x1) * 0.45;
  return `M${x1} ${y1} C${x1 + pull} ${y1} ${x2 - pull} ${y2} ${x2} ${y2}`;
}

type Tone = "phone" | "buds" | "busy" | "idle";

/** Light travelling along a live link. */
function Pulses({ path, tone }: { path: string; tone: Tone }) {
  if (tone === "idle") return null;
  const duration = tone === "busy" ? 2.4 : 1.5;
  return (
    <g className={`hero-pulses hero-pulses--${tone}`}>
      {[0, 1, 2].map((i) => (
        <g key={i} opacity="0">
          <circle className="hero-pulse__halo" r="5" />
          <circle className="hero-pulse__core" r="2.1" />
          <animateMotion
            dur={`${duration}s`}
            begin={`${(-duration * i) / 3}s`}
            repeatCount="indefinite"
            path={path}
          />
          <animate
            attributeName="opacity"
            values="0;1;1;0"
            keyTimes="0;0.15;0.8;1"
            dur={`${duration}s`}
            begin={`${(-duration * i) / 3}s`}
            repeatCount="indefinite"
          />
        </g>
      ))}
    </g>
  );
}

function Phone({
  slot,
  phone,
  ids,
}: {
  slot: Slot;
  phone: SourceView | null;
  ids: { screen: string; art: string };
}) {
  const { x, y, w, h, tilt } = slot;
  const playing = phone?.state === "streaming";
  const busy = phone?.state === "connecting" || phone?.state === "waiting";
  return (
    <g
      className={`hero-phone ${playing ? "hero-phone--on" : busy ? "hero-phone--busy" : ""}`}
      transform={`translate(${x} ${y}) rotate(${tilt})`}
    >
      <rect className="hero-phone__body" x={-w / 2} y={-h / 2} width={w} height={h} rx="7" />
      <rect
        className="hero-phone__screen"
        x={-w / 2 + 2.4}
        y={-h / 2 + 2.4}
        width={w - 4.8}
        height={h - 4.8}
        rx="5"
        fill={playing ? `url(#${ids.screen})` : undefined}
      />
      <circle className="hero-phone__camera" cx="0" cy={-h / 2 + 6} r="1.3" />
      {playing && (
        <g className="hero-phone__player">
          <rect x={-w / 2 + 6} y={-h / 2 + 12} width={w - 12} height={w - 12} rx="3" fill={`url(#${ids.art})`} />
          <g className="hero-phone__note" transform={`translate(0 ${-h / 2 + 12 + (w - 12) / 2})`}>
            <path d="M-2.2 3 V-4.2 L3.6 -5.6 V1.6" />
            <ellipse cx="-3.6" cy="3.2" rx="1.9" ry="1.5" />
            <ellipse cx="2.2" cy="1.8" rx="1.9" ry="1.5" />
          </g>
          <rect className="hero-phone__track" x={-w / 2 + 6} y={h / 2 - 15} width={w - 12} height="2" rx="1" />
          <rect className="hero-phone__progress" x={-w / 2 + 6} y={h / 2 - 15} width={w - 12} height="2" rx="1" />
          <rect className="hero-phone__line" x={-w / 2 + 6} y={h / 2 - 10} width={(w - 12) * 0.6} height="1.6" rx="0.8" />
        </g>
      )}
    </g>
  );
}

/** One earbud, stem down. The right one is mirrored. */
function Bud({ side, layer, ids }: { side: "a" | "b"; layer: "front" | "back"; ids: { shell: string; stem: string } }) {
  return (
    <g className={`hero-bud hero-bud--${side} hero-bud--${layer}`}>
      <g transform={side === "b" ? "scale(-1 1)" : undefined}>
        <rect className="hero-bud__stem" x="0.6" y="2.5" width="5.6" height="20" rx="2.8" fill={`url(#${ids.stem})`} transform="rotate(-9)" />
        <rect className="hero-bud__tip" x="0.6" y="20.3" width="5.6" height="2.4" rx="1.1" transform="rotate(-9)" />
        <ellipse className="hero-bud__shell" cx="0" cy="0" rx="7.2" ry="7.9" fill={`url(#${ids.shell})`} />
        <ellipse className="hero-bud__grille" cx="-2.6" cy="0.8" rx="2.5" ry="3.1" />
        <ellipse className="hero-bud__shine" cx="1.8" cy="-3.4" rx="2.4" ry="1.4" />
      </g>
    </g>
  );
}

/**
 * Phone(s) → laptop → buds, with light travelling along every link that is
 * live. A pure function of the current state, like InverterBar's heroes.
 */
export function FlowHero({ device, phones }: FlowHeroProps) {
  const uid = useId().replace(/:/g, "");
  const ids = {
    screen: `${uid}-screen`,
    art: `${uid}-art`,
    laptop: `${uid}-laptop`,
    clip: `${uid}-clip`,
    shell: `${uid}-shell`,
    stem: `${uid}-stem`,
    glow: `${uid}-glow`,
  };

  const budsOn = device?.state === "connected";
  const budsBusy = device?.state === "connecting" || device?.state === "disconnecting";
  const shown = shownPhones(phones);
  const placed = placePhones(shown);
  const enabled = phones.filter((phone) => phone.enabled);
  const playing = phones.filter((phone) => phone.state === "streaming");
  const more = Math.max(0, enabled.length - MAX_PHONES_DRAWN);

  const phoneName =
    enabled.length >= 2 ? `${enabled.length} phones` : shown[0] ? shortName(shown[0].name) : "Phone";
  const phoneState =
    enabled.length >= 2
      ? playing.length === enabled.length
        ? "Playing"
        : playing.length > 0
          ? `${playing.length} playing`
          : "Connecting"
      : shown[0]
        ? PHONE_STATE[shown[0].state]
        : "Not paired";
  const laptopDetail = budsOn
    ? playing.length >= 2
      ? "Mixing phones + laptop"
      : playing.length === 1
        ? "Mixing phone + laptop"
        : "Playing to buds"
    : "Idle";

  const description = [
    phones.length > 0
      ? phones.map((phone) => `${phone.name}: ${bridgeStatusLabel(phone.state).toLowerCase()}`).join(". ")
      : "No phone paired",
    device ? `${device.name}: ${statusLabel(device).toLowerCase()}` : "No buds paired",
  ].join(". ");

  const budsPath = link(LAPTOP.right, LAPTOP.mid, BUDS.x - 20, BUDS.y + 2);
  const budsTone: Tone = budsOn ? "buds" : budsBusy ? "busy" : "idle";

  return (
    <svg className="hero" viewBox="0 0 360 142" role="img" aria-label={description}>
      <defs>
        <linearGradient id={ids.screen} x1="0" y1="0" x2="0.6" y2="1">
          <stop offset="0" className="hero-stop--phone-a" />
          <stop offset="1" className="hero-stop--phone-b" />
        </linearGradient>
        <linearGradient id={ids.art} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#ffd36e" />
          <stop offset="0.55" stopColor="#ff7a7a" />
          <stop offset="1" stopColor="#8a6dff" />
        </linearGradient>
        <linearGradient id={ids.laptop} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" className="hero-stop--screen-a" />
          <stop offset="1" className="hero-stop--screen-b" />
        </linearGradient>
        <clipPath id={ids.clip}>
          <rect x="130" y="18" width="100" height="61" rx="3" />
        </clipPath>
        <radialGradient id={ids.shell} cx="0.35" cy="0.3" r="0.85">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="0.7" stopColor="#eef0f3" />
          <stop offset="1" stopColor="#c9cdd4" />
        </radialGradient>
        <radialGradient id={ids.glow}>
          <stop offset="0" className="hero-stop--glow" />
          <stop offset="1" className="hero-stop--glow" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={ids.stem} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="0.6" stopColor="#eceef1" />
          <stop offset="1" stopColor="#c4c8cf" />
        </linearGradient>
      </defs>

      {/* links, under everything */}
      {placed.map(({ phone, slot }) => {
        const path = link(slot.x + slot.w / 2 + 1, slot.y, LAPTOP.left - 1, LAPTOP.mid);
        const tone: Tone =
          phone?.state === "streaming"
            ? "phone"
            : phone?.state === "connecting" || phone?.state === "waiting"
              ? "busy"
              : "idle";
        return (
          <g key={phone?.id ?? "none"}>
            <path className={`hero-link hero-link--${tone}`} d={path} />
            <Pulses path={path} tone={tone} />
          </g>
        );
      })}
      <path className={`hero-link hero-link--${budsTone}`} d={budsPath} />
      <Pulses path={budsPath} tone={budsTone} />

      {/* phones */}
      {placed.map(({ phone, slot }) => (
        <Phone key={phone?.id ?? "none"} slot={slot} phone={phone} ids={ids} />
      ))}
      {more > 0 && (
        <g className="hero-more">
          <circle cx="86" cy="18" r="9" />
          <text x="86" y="21.5">
            +{more}
          </text>
        </g>
      )}

      {/* laptop */}
      <g className={`hero-laptop ${budsOn ? "hero-laptop--on" : ""}`}>
        <rect className="hero-laptop__lid" x="126" y="14" width="108" height="69" rx="7" />
        <rect
          className="hero-laptop__screen"
          x="130"
          y="18"
          width="100"
          height="61"
          rx="3"
          fill={budsOn ? `url(#${ids.laptop})` : undefined}
        />
        <g clipPath={`url(#${ids.clip})`}>
          {budsOn ? (
            <>
              <path className="hero-wave hero-wave--laptop" d={wave(130, 100, 48.5, 6, 24)} />
              {playing.length > 0 && (
                <path className="hero-wave hero-wave--phone" d={wave(130, 100, 48.5, 4.5, 17)} />
              )}
            </>
          ) : (
            <path className="hero-wave hero-wave--idle" d="M136 48.5 H224" />
          )}
        </g>
        <path className="hero-laptop__base" d="M110 85 H250 L244.5 92 Q243 93.5 240.5 93.5 H119.5 Q117 93.5 115.5 92 Z" />
        <rect className="hero-laptop__notch" x="170" y="85" width="20" height="2.6" rx="1.3" />
      </g>

      {/* buds */}
      <g
        className={`hero-buds ${budsOn ? "hero-buds--on" : budsBusy ? "hero-buds--busy" : ""}`}
        transform={`translate(${BUDS.x} ${BUDS.y})`}
      >
        <ellipse className="hero-buds__glow" cx="0" cy="27" rx="24" ry="5" fill={`url(#${ids.glow})`} />
        <Bud side="a" layer="back" ids={ids} />
        <Bud side="b" layer="back" ids={ids} />
        <Bud side="a" layer="front" ids={ids} />
        <Bud side="b" layer="front" ids={ids} />
      </g>

      {/* labels */}
      <g className="hero-labels">
        <text x="52" y="116" className="hero-name">
          {phoneName}
        </text>
        <text x="52" y="131" className="hero-state">
          {phoneState}
        </text>
        <text x="180" y="116" className="hero-name">
          This laptop
        </text>
        <text x="180" y="131" className="hero-state">
          {laptopDetail}
        </text>
        <text x="310" y="116" className="hero-name">
          {device ? shortName(device.name) : "Buds"}
        </text>
        <text x="310" y="131" className="hero-state">
          {device ? statusLabel(device) : "Not paired"}
        </text>
      </g>
    </svg>
  );
}
