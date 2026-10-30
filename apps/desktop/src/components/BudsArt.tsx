import { useId } from "react";

export type BudsArtState = "connected" | "busy" | "off";

interface BudsArtProps {
  state: BudsArtState;
  size?: number;
}

/**
 * A pair of true-wireless earbuds, drawn generically (no brand): one bud in
 * front, one behind, with a soft glow and sound rings while connected.
 */
export function BudsArt({ state, size = 132 }: BudsArtProps) {
  const id = useId().replace(/:/g, "");
  const shell = `buds-shell-${id}`;
  const stem = `buds-stem-${id}`;
  const glow = `buds-glow-${id}`;

  return (
    <svg
      className={`buds-art buds-art--${state}`}
      viewBox="0 0 160 160"
      width={size}
      height={size}
      aria-hidden="true"
    >
      <defs>
        <radialGradient id={glow} cx="50%" cy="52%" r="50%">
          <stop offset="0%" className="buds-art__glow-a" />
          <stop offset="100%" className="buds-art__glow-b" />
        </radialGradient>
        <linearGradient id={shell} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" className="buds-art__shell-a" />
          <stop offset="100%" className="buds-art__shell-b" />
        </linearGradient>
        <linearGradient id={stem} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" className="buds-art__shell-b" />
          <stop offset="55%" className="buds-art__shell-a" />
          <stop offset="100%" className="buds-art__shell-b" />
        </linearGradient>
      </defs>

      <circle className="buds-art__halo" cx="80" cy="82" r="72" fill={`url(#${glow})`} />

      <g className="buds-art__rings">
        <circle cx="80" cy="82" r="50" />
        <circle cx="80" cy="82" r="62" />
      </g>

      {/* back bud, mirrored and smaller */}
      <g className="buds-art__bud buds-art__bud--back" transform="translate(108 36) rotate(-16) scale(-0.8 0.8)">
        <rect x="-7" y="30" width="14" height="52" rx="7" fill={`url(#${stem})`} />
        <ellipse cx="0" cy="22" rx="24" ry="22" fill={`url(#${shell})`} />
        <ellipse className="buds-art__tip" cx="-15" cy="17" rx="10" ry="12" />
        <rect className="buds-art__band" x="-7" y="74" width="14" height="6" rx="3" />
      </g>

      {/* front bud */}
      <g className="buds-art__bud buds-art__bud--front" transform="translate(58 46) rotate(-14)">
        <rect x="-8" y="32" width="16" height="58" rx="8" fill={`url(#${stem})`} />
        <ellipse cx="0" cy="24" rx="27" ry="25" fill={`url(#${shell})`} />
        <ellipse className="buds-art__tip" cx="-17" cy="19" rx="11" ry="13" />
        <ellipse className="buds-art__shine" cx="8" cy="12" rx="9" ry="5" />
        <circle className="buds-art__sensor" cx="11" cy="30" r="2.2" />
        <rect className="buds-art__band" x="-8" y="82" width="16" height="6" rx="3" />
      </g>
    </svg>
  );
}
