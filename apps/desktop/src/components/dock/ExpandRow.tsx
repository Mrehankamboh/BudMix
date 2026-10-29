import { ChevronRight } from "lucide-react";
import { useId, type ReactNode } from "react";

interface ExpandRowProps {
  title: string;
  value: string;
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
}

/** A row that opens a section in place, like InverterBar's "Today" and "Details". */
export function ExpandRow({ title, value, open, onToggle, children }: ExpandRowProps) {
  const id = useId();
  return (
    <div className="expand">
      <button
        type="button"
        className="expand__row"
        aria-expanded={open}
        aria-controls={id}
        onClick={onToggle}
      >
        <span className="expand__title">{title}</span>
        <span className="expand__value">{value}</span>
        <ChevronRight className={`expand__chevron ${open ? "expand__chevron--open" : ""}`} size={14} />
      </button>
      {open && (
        <div className="expand__body" id={id}>
          {children}
        </div>
      )}
    </div>
  );
}
