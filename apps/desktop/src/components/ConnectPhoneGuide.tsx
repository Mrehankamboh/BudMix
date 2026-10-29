/** "How to connect my phone": simple steps for Infinix XOS and other Android phones. */
export function ConnectPhoneGuide() {
  return (
    <section className="panel guide" aria-labelledby="guide-title">
      <h2 id="guide-title">How to connect my phone</h2>

      <details open>
        <summary>Over Wi-Fi (Cast) — Infinix Hot 11 Play / XOS</summary>
        <ol>
          <li>Turn on Wi-Fi on the phone (it doesn&apos;t matter which network).</li>
          <li>
            Swipe down twice and tap <strong>Cast</strong> (or <strong>Screen cast</strong>). Not there? Tap
            the pencil icon and drag the tile in, or open Settings and search for <em>cast</em>.
          </li>
          <li>
            List empty? Tap <strong>⋮</strong> (top right) and tick <strong>Enable wireless display</strong>.
          </li>
          <li>
            Tap <strong>BudsLink</strong>. The first time, allow the phone on the laptop.
          </li>
          <li>Play something: the phone goes quiet and the sound is in your buds.</li>
        </ol>
        <p className="muted">
          Ringtones and notification sounds keep playing on the phone. Some apps (e.g. Netflix) block casting.
        </p>
      </details>

      <details>
        <summary>Over Wi-Fi — other Android phones</summary>
        <ul>
          <li>Samsung: Quick settings → Smart View → BudsLink.</li>
          <li>Xiaomi / Redmi / POCO: Settings → Connection &amp; sharing → Cast.</li>
          <li>OPPO / realme / OnePlus: Settings → Connected devices → Screencast.</li>
          <li>Google Pixel: no Miracast. Use Bluetooth below.</li>
        </ul>
      </details>

      <details>
        <summary>Over Bluetooth (automatic)</summary>
        <ol>
          <li>Pair the phone once: Windows Settings → Bluetooth &amp; devices → Add device.</li>
          <li>
            Leave &quot;Phone connection&quot; on <strong>Auto</strong>: BudsLink connects to the phone by itself
            whenever it is not casting. Nothing to tap on the phone.
          </li>
        </ol>
      </details>

      <details>
        <summary>The phone doesn&apos;t see BudsLink</summary>
        <ul>
          <li>Keep BudsLink running and Wi-Fi on on both devices.</li>
          <li>Run the setup check above: the network must be Private and the Wi-Fi card must support Miracast.</li>
          <li>Turn the phone&apos;s Wi-Fi off and on, then open Cast again.</li>
          <li>Still nothing? Bluetooth always works: set Phone connection to Auto or Bluetooth only.</li>
        </ul>
      </details>
    </section>
  );
}
