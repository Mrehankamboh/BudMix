import type { ReactNode } from "react";

interface TileProps {
  icon: ReactNode;
  caption: string;
  value: string;
  detail: string;
  /** Highlights the tile when the thing it stands for is on. */
  tone?: "buds" | "phone" | "busy" | "off";
  label: string;
  onClick?: () => void;
}

/** One of the four status tiles; each is also the quickest way to change it. */
export function Tile({ icon, caption, value, detail, tone = "off", label, onClick }: TileProps) {
  return (
    <button
      type="button"
      className={`tile tile--${tone}`}
      aria-label={label}
      onClick={onClick}
      disabled={!onClick}
    >
      <span className="tile__icon" aria-hidden="true">
        {icon}
      </span>
      <span className="tile__text">
        <span className="tile__caption">{caption}</span>
        <span className="tile__value">{value}</span>
        <span className="tile__detail">{detail}</span>
      </span>
    </button>
  );
}
