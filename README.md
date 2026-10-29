# BudsLink

**Hear your laptop and your phone in the same Bluetooth earbuds at the same time, even buds without multipoint. Nothing to install on the phone.**

> **Status: early development (milestone M0).** Right now the repository holds
> the shared core (from BudsBridge) and a probe that checks whether this
> laptop can receive a phone's Miracast cast. The tray app comes in M1.

## How it works

```
              Wi-Fi Direct (Miracast cast)              Bluetooth (A2DP)
[Phone] ─────────────────────────────────► [Laptop: BudsLink] ─────────────► [Buds]
   │          fallback: Bluetooth (A2DP)        ▲   Windows mixes every
   └────────────────────────────────────────────┤   stream into the buds
                                                │
                       the laptop's own sound (YouTube, Zoom, ...)
```

- The buds connect only to the laptop (they can hold just one link).
- **Phone over Wi-Fi:** the phone uses Android's built-in *Cast* (Miracast).
  BudsLink is the receiver, so the phone's audio plays on the laptop and goes
  to the buds. The phone user has to tap **Cast → BudsLink**; Android does not
  let any other device pull audio from a phone by itself.
- **Phone over Bluetooth (fallback):** the laptop acts as a Bluetooth speaker
  for the paired phone, and BudsLink opens that link itself, so no tap is
  needed on the phone.
- Windows' audio engine mixes everything sent to the buds.

## Roadmap

| Milestone | Scope | Status |
|---|---|---|
| M0 | Miracast receiver probe (`cast-probe`) | code written, awaiting first build and hardware test |
| M1 | Tray app: buds auto-connect/reconnect, laptop audio in buds, test tone, demo mode | planned |
| M2 | Setup wizard and diagnostics | planned |
| M3 | Miracast receiver inside the app, video window, approvals | planned |
| M4 | Bluetooth phone fallback, Auto Wi-Fi/Bluetooth switching, per-source volume | planned |
| M5 | Reliability polish, installer, docs | planned |
| M6 | Optional AirPlay receiver for iPhone guests | optional |

## Build (developers)

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). Testing on real hardware:
[docs/HARDWARE_TESTING.md](docs/HARDWARE_TESTING.md). Connecting an
Infinix/Android phone: [docs/CONNECT_PHONE.md](docs/CONNECT_PHONE.md).

## Honest notes

- Miracast adds roughly 100–300 ms and Bluetooth to the buds another
  150–250 ms, so video watched on the phone's own screen will be slightly out
  of lip-sync.
- Some laptop Wi-Fi cards cannot receive Miracast; then the phone uses
  Bluetooth.
- Some DRM apps (for example Netflix) may block casting.
- Ringtones and notification sounds usually keep playing on the phone itself;
  Android sends media audio to the cast.

## Credits and license

BudsLink is based on **[BudsBridge](https://github.com/shakaibarsalan/BudsBridge)**
by Muhammad Shakaib Arsalan (MIT): the device model, connection and bridge
state machines, reconnect policies, settings, the Windows Bluetooth/Core Audio
backend and the `bt-audio` tool come from it. The Bluetooth connect technique
follows [ToothTray](https://github.com/m2jean/ToothTray) (BSD-2-Clause).

MIT. See [LICENSE](LICENSE), which keeps the BudsBridge copyright notice.
