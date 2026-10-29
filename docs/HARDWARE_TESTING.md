# Hardware testing

CI has no Bluetooth or Wi-Fi Direct hardware, so these checks run on a real
laptop, phone and buds. Write the result next to each item.

Reference setup: Windows 11 laptop (Intel Dual Band Wireless-AC 8265), Infinix
Hot 11 Play (Android 11, XOS), "TWS" buds without multipoint.

## M0: Miracast receiver probe

Run in a terminal in the project folder:

```powershell
cargo run -p budslink-miracast --example cast-probe
```

| ID | Check | Expected | Result |
|---|---|---|---|
| M0-1 | The probe starts | `verdict: Ready` and `listening=Listening`. If it prints `FAILED`, copy the whole output | |
| M0-2 | The phone finds the laptop | Infinix Cast list shows **BudsLink** within ~10 s | |
| M0-3 | Infinix menu names | Write down the real path to Cast and to "Enable wireless display" | |
| M0-4 | The phone connects | The probe prints `phone connecting: ...` then `stream from ... is playing` | |
| M0-5 | Audio plays on the laptop | Phone audio comes out of the laptop's default output (buds if they are connected and are the default) | |
| M0-6 | Phone speaker | The phone's own speaker goes quiet while casting | |
| M0-7 | Disconnect and recast | Disconnect on the phone; the probe prints `phone disconnected`; casting again works without restarting the probe | |
| M0-8 | Latency (rough) | Tap a video on the phone and estimate the delay until it is heard | |

If M0-1 says `AccessDenied`, the unpackaged app is not allowed to use the
receiver API; BudsLink then moves to plan B (package identity) or plan C
(Windows' built-in Wireless Display app). If it says
`WifiCardCannotReceive`, the phone will use Bluetooth only.

## M1: buds, laptop audio and the tray app

Find your buds' id first:

```powershell
cargo run -p budslink-win --example bt-audio -- list
```

| ID | Check | Expected | Result |
|---|---|---|---|
| M1-1 | `bt-audio -- list` | "TWS" is listed with `[Stereo, HandsFree]`; the id looks like `{xxxxxxxx-...}` | |
| M1-2 | `bt-audio -- connect "<id>"` (buds out of the case, not connected to the phone) | `done in N s`; write down N | |
| M1-3 | `bt-audio -- status "<id>"` | `buds output: Headphones (TWS)`; `call mode: no` | |
| M1-4 | `bt-audio -- make-default "<id>"` | Windows' sound icon now shows the buds; YouTube in the browser plays in the buds | |
| M1-5 | `bt-audio -- tone "<id>"` | A short, clean beep in the buds: no click at the start or the end | |
| M1-6 | Start the app (`pnpm tauri dev` in `apps/desktop`), click the tray icon | The dock opens; "Laptop audio plays in your buds" | |
| M1-7 | Dock → **Test beep** | Beep in the buds; the button shows "Playing…" meanwhile | |
| M1-8 | Note the default output, then put the buds in the case | Within a few seconds the previous default (e.g. laptop speakers) is the default again | |
| M1-9 | Take the buds out again | BudsLink reconnects them (1 s, 2 s, 4 s ...) and laptop audio moves back to the buds | |
| M1-10 | Pick the laptop speakers yourself in Windows while the buds stay connected | BudsLink leaves your choice alone (it does not switch back) | |
| M1-11 | Open Windows Voice Recorder (or a Zoom test call) using the buds' mic | The dock warns "Buds switched to call mode (HFP)"; it goes away when the recorder closes | |
| M1-12 | Settings → turn off "Play laptop audio in my buds" | The previous default comes back | |
| M1-13 | Quit from the tray menu | The previous default comes back | |
| M1-14 | Demo mode: `$env:BUDSLINK_DEMO = "1"; pnpm tauri dev` | Fictional "Demo Buds Pro"; connecting them shows laptop audio "in your buds" | |

## M2: setup check and diagnostics

| ID | Check | Expected | Result |
|---|---|---|---|
| M2-1 | First start of the app | The settings window opens with "Welcome: let's check this PC" | |
| M2-2 | Bluetooth row | "Bluetooth is on". Turn Bluetooth off and click **Check again**: "Bluetooth is off" with **Open settings** | |
| M2-3 | Buds / phone rows | "Buds paired: TWS" and "Phone paired: Infinix HOT 11 Play" | |
| M2-4 | Network row on the Public home Wi-Fi | "Network is Public" with **Make Private (admin)**. Click it, say Yes to Windows: the row turns "Network is Private" | |
| M2-5 | Cancel the admin prompt instead | "The admin prompt was cancelled, so nothing changed." | |
| M2-6 | Firewall row | **Allow (admin)** adds the rule; the row turns OK. `wf.msc` shows "BudsLink (Private networks)", Private profile only | |
| M2-7 | Miracast row | "Wi-Fi card can receive Miracast" (Intel AC 8265) | |
| M2-8 | Test beep row | **Play beep** is heard in the buds; **Yes** marks it OK | |
| M2-9 | **Done** / **Finish later** | Next start opens the dock, not the wizard; the check stays available in settings | |
| M2-10 | Troubleshooting → Copy diagnostics | The text includes Windows build, Bluetooth radio, networks with their category, firewall, Miracast sink, laptop audio and Bluetooth devices | |
