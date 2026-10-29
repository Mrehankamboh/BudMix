import { useState } from "react";

import { api } from "../lib/api";

/** Copies what BudsLink and Windows see, for a bug report. */
export function DiagnosticsPanel() {
  const [report, setReport] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);

  const copy = async () => {
    setStatus("Reading what Windows reports…");
    let text: string;
    try {
      text = await api.diagnostics();
    } catch (err) {
      setStatus(`Couldn't read diagnostics: ${String(err)}`);
      return;
    }
    setReport(text);
    try {
      await navigator.clipboard.writeText(text);
      setStatus("Copied. Paste it into your message or bug report.");
    } catch {
      setStatus("Couldn't copy automatically. Select the text below and copy it.");
    }
  };

  return (
    <section className="panel" aria-labelledby="diagnostics-title">
      <h2 id="diagnostics-title">Troubleshooting</h2>
      <p className="muted small">
        A phone missing or not connecting? Copy what BudsLink and Windows see about your Bluetooth
        devices and send it with your report. It includes device names and Bluetooth addresses.
      </p>
      <button type="button" className="button" onClick={() => void copy()}>
        Copy diagnostics
      </button>
      {status && (
        <p className="muted small" role="status">
          {status}
        </p>
      )}
      {report !== null && (
        <textarea className="report" readOnly value={report} rows={8} aria-label="Diagnostics" />
      )}
    </section>
  );
}
