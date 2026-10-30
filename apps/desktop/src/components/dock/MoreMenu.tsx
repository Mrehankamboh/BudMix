import { Check, MoreHorizontal } from "lucide-react";
import { useEffect, useRef, useState } from "react";

interface MoreMenuProps {
  startWithWindows: boolean;
  onStartWithWindows: (enabled: boolean) => void;
  onSettings: () => void;
  onQuit: () => void;
}

/** The ⋯ menu in the dock header. */
export function MoreMenu({ startWithWindows, onStartWithWindows, onSettings, onQuit }: MoreMenuProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);

  const choose = (action: () => void) => () => {
    setOpen(false);
    action();
  };

  return (
    <div className="menu" ref={ref}>
      <button
        type="button"
        className="icon-button"
        aria-label="More"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <MoreHorizontal size={16} />
      </button>
      {open && (
        <div className="menu__list" role="menu">
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={startWithWindows}
            className="menu__item"
            onClick={choose(() => onStartWithWindows(!startWithWindows))}
          >
            <span className="menu__check">{startWithWindows && <Check size={13} />}</span>
            Start with Windows
          </button>
          <button type="button" role="menuitem" className="menu__item" onClick={choose(onSettings)}>
            <span className="menu__check" />
            All settings…
          </button>
          <div className="menu__separator" role="separator" />
          <button type="button" role="menuitem" className="menu__item" onClick={choose(onQuit)}>
            <span className="menu__check" />
            Quit BudMix
          </button>
        </div>
      )}
    </div>
  );
}
