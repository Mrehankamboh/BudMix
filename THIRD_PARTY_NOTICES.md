# Third-party notices

BudMix is MIT licensed. It builds on the following free software.

| Project | Used for | License |
|---|---|---|
| [BudsBridge](https://github.com/shakaibarsalan/BudsBridge) (Muhammad Shakaib Arsalan) | Base of the core, Windows backend and desktop app | MIT |
| [ToothTray](https://github.com/m2jean/ToothTray) (Jing Xia) | Technique for Bluetooth connect/disconnect via `KSPROPSETID_BtAudio` (no code copied) | BSD-2-Clause |
| [windows-rs](https://github.com/microsoft/windows-rs) (`windows`, `windows-core`, `windows-collections`, `windows-numerics`) | Windows APIs | MIT or Apache-2.0 |
| [Tauri](https://tauri.app) and plugins (autostart, global-shortcut, log, single-instance, notification) | Desktop shell | MIT or Apache-2.0 |
| serde, serde_json, log, thiserror | Settings, logging, errors | MIT or Apache-2.0 |
| React, React DOM | UI | MIT |
| lucide-react | Icons | ISC |
| Vite, Vitest, Testing Library, TypeScript, jsdom | Build and tests (not shipped) | MIT / Apache-2.0 |

No GPL code is included. The optional AirPlay receiver (milestone M6) would
likely use UxPlay (GPL-3.0) as a separate program; that will be decided
before it is built.
