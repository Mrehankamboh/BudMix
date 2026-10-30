import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { api } from "../lib/api";
import type { CheckResult, CheckStatus, Fix } from "../lib/types";

const STATUS_LABEL: Record<CheckStatus, string> = {
  ok: "OK",
  warning: "Needs attention",
  problem: "Problem",
  optional: "Optional",
  unknown: "Unknown",
};

function StatusIcon({ status }: { status: CheckStatus }) {
  const props = { size: 15, "aria-hidden": true } as const;
  switch (status) {
    case "ok":
      return <CheckCircle2 {...props} />;
    case "warning":
      return <AlertTriangle {...props} />;
    case "problem":
      return <XCircle {...props} />;
    default:
      return <Info {...props} />;
  }
}

/** The button text for a fix. "(admin)" warns that Windows will ask first. */
export function fixLabel(fix: Fix): string {
  switch (fix.kind) {
    case "openSettings":
      return "Open settings";
    case "makeNetworkPrivate":
      return "Make Private (admin)";
    case "addFirewallRule":
      return "Allow (admin)";
    case "installWirelessDisplay":
      return "Install (admin)";
  }
}

type Beep = "untested" | "playing" | "heard" | "silent";

interface SetupPanelProps {
  /** Finished once before: shown as a re-runnable check instead of a first-run wizard. */
  done: boolean;
  onDone: () => void;
  onPlayTone: () => Promise<void>;
}

/** Checks and fixes everything BudMix needs from this PC. */
export function SetupPanel({ done, onDone, onPlayTone }: SetupPanelProps) {
  const [checks, setChecks] = useState<CheckResult[] | null>(null);
  const [running, setRunning] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [beep, setBeep] = useState<Beep>("untested");

  const load = useCallback(async () => {
    setChecks(null);
    try {
      setChecks(await api.setupChecks());
    } catch (err) {
      setMessage(`Couldn't run the checks: ${String(err)}`);
      setChecks([]);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const fix = async (check: CheckResult) => {
    if (!check.fix) return;
    setRunning(check.id);
    setMessage(null);
    try {
      await api.runFix(check.fix);
      // Settings pages change things outside BudMix; check again after.
      if (check.fix.kind !== "openSettings") await load();
    } catch (err) {
      setMessage(String(err));
    } finally {
      setRunning(null);
    }
  };

  const playBeep = async () => {
    setBeep("playing");
    try {
      await onPlayTone();
      setBeep("untested");
    } catch {
      setBeep("silent");
    }
  };

  const problems = checks?.filter((check) => check.status === "problem").length ?? 0;
  // The beep counts as one more item once answered.
  const total = (checks?.length ?? 0) + 1;
  const ready =
    (checks?.filter((check) => check.status === "ok" || check.status === "optional").length ?? 0) +
    (beep === "heard" ? 1 : 0);

  return (
    <section className="section setup" aria-labelledby="setup-title">
      <header className="section__head">
        <div>
          <h2 id="setup-title">{done ? "Setup check" : "Welcome: let's check this PC"}</h2>
          <p className="section__lead">
            {done
              ? "Everything BudMix needs from Windows, at a glance."
              : 'BudMix checks what it needs and fixes it for you. Buttons marked "(admin)" show one Windows prompt.'}
          </p>
        </div>
        {checks !== null && (
          <p className="setup__score" aria-label={`${ready} of ${total} ready`}>
            <strong>{ready}</strong>
            <span>/{total} ready</span>
          </p>
        )}
      </header>
      {checks !== null && (
        <div className="progress" aria-hidden="true">
          <span style={{ width: `${Math.round((ready / total) * 100)}%` }} />
        </div>
      )}
      {message && (
        <p className="banner banner--error" role="alert">
          {message}
        </p>
      )}

      {checks === null ? (
        <p className="muted">Checking this PC…</p>
      ) : (
        <ul className="checks">
          {checks.map((check) => (
            <li key={check.id} className={`check check--${check.status}`} aria-label={check.title}>
              <span className="check__icon" title={STATUS_LABEL[check.status]}>
                <StatusIcon status={check.status} />
              </span>
              <span className="check__text">
                <span className="check__title">{check.title}</span>
                {check.detail && <small>{check.detail}</small>}
              </span>
              {check.fix && (
                <button
                  type="button"
                  className="link-button"
                  disabled={running !== null}
                  onClick={() => void fix(check)}
                >
                  {running === check.id ? "Working…" : fixLabel(check.fix)}
                </button>
              )}
            </li>
          ))}
          <li className={`check check--${beep === "heard" ? "ok" : beep === "silent" ? "problem" : "unknown"}`} aria-label="Test beep">
            <span className="check__icon">
              <StatusIcon status={beep === "heard" ? "ok" : beep === "silent" ? "problem" : "unknown"} />
            </span>
            <span className="check__text">
              <span className="check__title">
                {beep === "heard" ? "Beep heard in the buds" : "Do you hear the beep in your buds?"}
              </span>
              {beep === "silent" && (
                <small>
                  Check that the buds are connected and not in the case, then turn up the volume.
                  If the sound came from the laptop, turn on "Play laptop audio in my buds".
                </small>
              )}
            </span>
            {beep !== "heard" && (
              <span className="check__actions">
                <button
                  type="button"
                  className="link-button"
                  disabled={beep === "playing"}
                  onClick={() => void playBeep()}
                >
                  {beep === "playing" ? "Playing…" : "Play beep"}
                </button>
                <button type="button" className="link-button" onClick={() => setBeep("heard")}>
                  Yes
                </button>
                <button type="button" className="link-button" onClick={() => setBeep("silent")}>
                  No
                </button>
              </span>
            )}
          </li>
        </ul>
      )}

      <div className="setup__actions">
        <button type="button" className="button" disabled={checks === null} onClick={() => void load()}>
          Check again
        </button>
        {!done && (
          <button type="button" className="button button--accent" onClick={onDone}>
            {problems > 0 ? "Finish later" : "Done"}
          </button>
        )}
      </div>
    </section>
  );
}
