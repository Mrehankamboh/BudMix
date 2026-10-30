import { useState, type ReactNode } from "react";

type Tab = "wifi" | "others" | "bluetooth" | "help";

const TABS: { id: Tab; label: string }[] = [
  { id: "wifi", label: "Wi-Fi (Infinix)" },
  { id: "others", label: "Other Android" },
  { id: "bluetooth", label: "Bluetooth" },
  { id: "help", label: "Not working?" },
];

function Steps({ children }: { children: ReactNode }) {
  return <ol className="steps">{children}</ol>;
}

/** "How to connect my phone": simple steps for Infinix XOS and other Android phones. */
export function ConnectPhoneGuide() {
  const [tab, setTab] = useState<Tab>("wifi");

  return (
    <section className="section guide" aria-labelledby="guide-title">
      <header className="section__head">
        <div>
          <h2 id="guide-title">How to connect my phone</h2>
          <p className="section__lead">Nothing to install on the phone. Pick how it should send its sound.</p>
        </div>
      </header>

      <div className="tabs" role="tablist" aria-label="Ways to connect">
        {TABS.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            id={`guide-tab-${item.id}`}
            aria-selected={tab === item.id}
            aria-controls="guide-panel"
            className="tabs__tab"
            onClick={() => setTab(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>

      <div className="guide__panel" role="tabpanel" id="guide-panel" aria-labelledby={`guide-tab-${tab}`}>
        {tab === "wifi" && (
          <>
            <Steps>
              <li>Turn on Wi-Fi on the phone. Any network, or none, is fine.</li>
              <li>
                Swipe down twice and tap <strong>Cast</strong> (or <strong>Screen cast</strong>). Not there? Tap
                the pencil icon and drag the tile in, or search Settings for <em>cast</em>.
              </li>
              <li>
                List empty? Tap <strong>⋮</strong> and tick <strong>Enable wireless display</strong>.
              </li>
              <li>
                Tap <strong>BudMix</strong>, then allow the phone on the laptop the first time.
              </li>
              <li>Play something. The phone goes quiet and the sound is in your buds.</li>
            </Steps>
            <p className="footnote">
              Ringtones and notifications keep playing on the phone. Some apps (e.g. Netflix) block casting.
            </p>
          </>
        )}

        {tab === "others" && (
          <dl className="brand-list">
            <dt>Samsung</dt>
            <dd>Quick settings → Smart View → BudMix</dd>
            <dt>Xiaomi · Redmi · POCO</dt>
            <dd>Settings → Connection &amp; sharing → Cast</dd>
            <dt>OPPO · realme · OnePlus</dt>
            <dd>Settings → Connected devices → Screencast</dd>
            <dt>Google Pixel</dt>
            <dd>No Miracast. Use Bluetooth instead.</dd>
          </dl>
        )}

        {tab === "bluetooth" && (
          <Steps>
            <li>Pair the phone once: Windows Settings → Bluetooth &amp; devices → Add device.</li>
            <li>
              Leave <strong>Phone connection</strong> on <strong>Auto</strong>. BudMix connects to the phone by
              itself whenever it is not casting.
            </li>
            <li>Play something on the phone. Nothing else to tap.</li>
          </Steps>
        )}

        {tab === "help" && (
          <ul className="tips">
            <li>Keep BudMix running and Wi-Fi on, on both devices.</li>
            <li>Run the setup check above: the network must be Private and the Wi-Fi card must support Miracast.</li>
            <li>Turn the phone&apos;s Wi-Fi off and on, then open Cast again.</li>
            <li>Still nothing? Bluetooth always works: set Phone connection to Auto or Bluetooth only.</li>
          </ul>
        )}
      </div>
    </section>
  );
}
