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
