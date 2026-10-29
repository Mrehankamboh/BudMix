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

# M0: does this laptop's built-in Miracast receiver work from BudsLink?
cargo run -p budslink-miracast --example cast-probe

# Bluetooth audio devices (from BudsBridge)
cargo run -p budslink-win --example bt-audio -- list
```

## Layout

```
crates/
  budslink-core/       device model, state machines, reconnect policy, settings (no OS code)
  budslink-win/        Windows backend: Bluetooth endpoints, connect/disconnect, phone audio
  budslink-miracast/   Miracast receiver; examples/cast-probe.rs
docs/                  development, hardware testing, connecting the phone
```
