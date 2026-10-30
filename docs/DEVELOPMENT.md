# Development

## Prerequisites (Windows 11, all free)

1. **Visual Studio 2022 Build Tools** with the **Desktop development with C++**
   workload (it includes the MSVC compiler and the Windows 11 SDK).
   ```powershell
   winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```
2. **Rust** (stable, MSVC toolchain):
   ```powershell
   winget install --id Rustlang.Rustup
   ```
   Open a new terminal afterwards and check with `cargo --version`.
3. **Node.js 22+** and **pnpm 10** (needed from M1 for the desktop app):
   ```powershell
   npm install -g pnpm
   ```

## Commands

```powershell
cargo test --workspace                 # unit tests (pure logic, no hardware)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# M0: does this laptop's built-in Miracast receiver work from BudMix?
cargo run -p budmix-miracast --example cast-probe

# Bluetooth audio devices, laptop audio and the test beep
cargo run -p budmix-win --example bt-audio -- list
cargo run -p budmix-win --example bt-audio -- status "{buds-id}"
cargo run -p budmix-win --example bt-audio -- tone "{buds-id}"
```

The desktop app (from `apps/desktop`):

```powershell
pnpm install
pnpm test            # UI tests (Vitest)
pnpm typecheck
pnpm tauri dev       # run the app
$env:BUDMIX_DEMO = "1"; pnpm tauri dev   # simulated buds and phone, no hardware
pnpm tauri build     # target/release/budmix.exe and the NSIS installer
```

## Layout

```
crates/
  budmix-core/       device model, state machines, reconnect policy, settings (no OS code)
  budmix-win/        Windows backend: Bluetooth endpoints, connect/disconnect, phone audio
  budmix-miracast/   Miracast receiver; examples/cast-probe.rs
apps/desktop/          Tauri 2 app: tray, dock, settings window (React UI in src/)
docs/                  development, hardware testing, connecting the phone
```
