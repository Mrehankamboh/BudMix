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
| M0-1 | The probe starts | `verdict: Ready` and `listening=Listening`. If it prints `FAILED`, copy the whole output | PASS on the reference laptop (2026-10-30): `wifi=Supported settings=Success start=Success listening=Listening` |
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

## M3: phone over Wi-Fi inside the app

Start the app (`pnpm tauri dev` in `apps/desktop`, or the installed BudsLink). The network must be Private (M2-4).

| ID | Check | Expected | Result |
|---|---|---|---|
| M3-1 | Dock → Sources → "Phone over Wi-Fi" | "Waiting · Ready: tap Cast → "BudsLink" on the phone" | |
| M3-2 | Infinix: Cast → **BudsLink** (first time) | Dock opens with "Infinix … wants to play in your buds"; a notification says the same | |
| M3-3 | **Allow always** | Phone audio plays in the buds; notification "Phone audio is now in your buds"; Sources shows "Infinix (Wi-Fi) · Playing" | |
| M3-4 | Laptop: play a YouTube video at the same time | Both are heard in the buds, mixed | |
| M3-5 | Phone screen window | A small window shows the phone screen; closing it hides it and the sound keeps playing; **Show phone screen** brings it back | |
| M3-6 | Disconnect on the phone, cast again | No question the second time (allowlist); plays at once | |
| M3-7 | Settings → Sources → Forget the phone, cast again | Asked again | |
| M3-8 | **Refuse** a cast | The phone disconnects | |
| M3-9 | Turn the network Public (Windows Settings) | Within ~10 s the receiver stops; Sources says "Network is Public: receiver off" | |

## M4: Bluetooth fallback, Auto switching, volumes

| ID | Check | Expected | Result |
|---|---|---|---|
| M4-1 | Phone connection = Auto, phone not casting, phone paired | BudsLink connects the phone over Bluetooth by itself; Sources "Infinix (Bluetooth) · Playing" when the phone plays | |
| M4-2 | While Bluetooth plays, start casting | Bluetooth shows "Paused · Held off while the phone casts"; no echo (the sound is heard once) | |
| M4-3 | Stop casting | About 3 s later Bluetooth comes back by itself | |
| M4-4 | Phone connection = Wi-Fi only | Bluetooth stays paused; casting works | |
| M4-5 | Phone connection = Bluetooth only | Phone over Wi-Fi shows Off; the phone no longer lists BudsLink | |
| M4-6 | Move each slider and mute button (Buds, Laptop, Phone Wi-Fi, Phone Bluetooth) | Only that source changes; Buds changes the Windows volume of the buds | |
| M4-7 | Quit BudsLink after lowering Laptop | Other apps get their own volume back | |

## M5: reliability

| ID | Check | Expected | Result |
|---|---|---|---|
| M5-1 | Sleep the laptop 5 minutes with buds connected, wake it | Buds reconnect (or connect when taken out); receiver ready again | |
| M5-2 | Turn Wi-Fi off and on | Receiver restarts by itself (Sources: Reconnecting → Waiting) | |
| M5-3 | Buds out of range for 1 minute, come back | Auto-reconnect brings them back; laptop audio returns to the buds | |
| M5-4 | Case open/close several times | Same as M5-3; previous output restored while they are away | |
| M5-5 | Restart BudsLink while casting | The phone reconnects or recasts; nothing plays twice | |
| M5-6 | Start with Windows on, reboot | BudsLink is in the tray, receiver ready, no window opened | |
| M5-7 | Installer `BudsLink_<version>_x64-setup.exe` | Installs for the current user without an admin prompt; Start menu entry; uninstaller works | |
| M5-8 | **1 hour**: laptop video + phone video (cast) together | No clicks, pops, crackles or drift; note any dropouts with the time | |

## Results

| Field | Value |
|---|---|
| Laptop / Wi-Fi card | Lenovo, Intel Dual Band Wireless-AC 8265 (Miracast Sink: Supported) |
| Windows build | 26300 |
| Buds | TWS |
| Phone | Infinix Hot 11 Play, Android 11, XOS |
| Latency (Wi-Fi / Bluetooth) | |
| Problems seen | |
