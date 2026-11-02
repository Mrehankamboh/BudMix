import { ShieldCheck } from "lucide-react";
import { useEffect, useState } from "react";

import { api } from "../../lib/api";
import type { CastStatus } from "../../lib/types";

type Step = "quiet" | "ask" | "working" | "done";

/**
 * A cast connects without sound while Windows Firewall stops it. Says so when
 * a phone casts, and adds the rule on one click, so nobody needs a command.
 */
export function CastSoundPrompt({ cast }: { cast: CastStatus | null }) {
  const casting = cast?.phase === "connecting" || cast?.phase === "playing";
  const [step, setStep] = useState<Step>("quiet");
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => {
    setStep("quiet");
    setProblem(null);
    if (!casting) return;
    let current = true;
    api
      .castSoundAllowed()
      .then((allowed) => {
        if (current && !allowed) setStep("ask");
      })
      .catch((err: unknown) => console.warn("firewall check failed", err));
    return () => {
      current = false;
    };
  }, [casting]);

  // The Windows prompt takes focus, which would otherwise close the dock.
  const waiting = step === "ask" || step === "working";
  useEffect(() => {
    api.dockKeepOpen(waiting).catch((err: unknown) => console.warn("dock keep-open failed", err));
  }, [waiting]);

  const allow = async () => {
    setStep("working");
    setProblem(null);
    try {
      await api.runFix({ kind: "addFirewallRule" });
      if (await api.castSoundAllowed()) {
        setStep("done");
      } else {
        setStep("ask");
        setProblem("Windows still blocks the sound. Details are in the setup check.");
      }
    } catch (err) {
      setStep("ask");
      setProblem(String(err));
    }
  };

  if (!casting || step === "quiet") return null;

  if (step === "done") {
    return (
      <p className="cast-banner" role="status">
        Sound allowed. If the phone stays silent, stop the cast and cast again.
      </p>
    );
  }

  const phone = cast?.phone ?? "the phone";
  return (
    <section className="cast-banner cast-banner--ask" aria-label="Allow the phone's sound">
      <p className="cast-banner__title">
        <ShieldCheck size={14} aria-hidden="true" />
        Allow the phone&apos;s sound?
      </p>
      <p className="cast-banner__note">
        Windows Firewall is stopping the sound from {phone}. BudMix can add the rule it needs. It is
        for BudMix only, for devices on the same local network, also on public Wi-Fi. Windows asks
        you to confirm once.
      </p>
      {problem && (
        <p className="cast-banner__note" role="alert">
          {problem}
        </p>
      )}
      <div className="cast-banner__actions">
        <button
          type="button"
          className="button button--accent"
          disabled={step === "working"}
          onClick={() => void allow()}
        >
          {step === "working" ? "Waiting for Windows…" : "Allow sound"}
        </button>
        <button
          type="button"
          className="button"
          disabled={step === "working"}
          onClick={() => setStep("quiet")}
        >
          Not now
        </button>
      </div>
    </section>
  );
}
