import { MonitorSmartphone } from "lucide-react";

import type { CastStatus } from "../../lib/types";

interface CastBannerProps {
  cast: CastStatus | null;
  onApprove: (remember: boolean) => void;
  onReject: () => void;
  onShowVideo: (visible: boolean) => void;
}

/** The casting phone: asks about a new one, or offers the picture window. */
export function CastBanner({ cast, onApprove, onReject, onShowVideo }: CastBannerProps) {
  if (!cast || cast.phase === "idle") return null;
  const phone = cast.phone ?? "A phone";

  if (cast.phase === "needsApproval") {
    return (
      <section className="cast-banner cast-banner--ask" aria-label="Phone wants to cast">
        <p className="cast-banner__title">
          <MonitorSmartphone size={14} aria-hidden="true" />
          {phone} wants to play in your buds
        </p>
        {cast.pin && (
          <p className="cast-banner__pin">
            PIN on the phone: <strong>{cast.pin}</strong>
          </p>
        )}
        <div className="cast-banner__actions">
          <button type="button" className="button button--accent" onClick={() => onApprove(true)}>
            Allow always
          </button>
          <button type="button" className="button" onClick={() => onApprove(false)}>
            Just this time
          </button>
          <button type="button" className="button" onClick={onReject}>
            Refuse
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="cast-banner" aria-label="Phone casting">
      <p className="cast-banner__title">
        <MonitorSmartphone size={14} aria-hidden="true" />
        {cast.phase === "playing" ? `${phone} is casting over Wi-Fi` : `${phone} is connecting…`}
      </p>
      <button type="button" className="link-button" onClick={() => onShowVideo(!cast.videoVisible)}>
        {cast.videoVisible ? "Hide phone screen" : "Show phone screen"}
      </button>
    </section>
  );
}
