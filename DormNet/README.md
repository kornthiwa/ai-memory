# DormNet (KIOIS Net)

Windows background agent for Netprasomplace MikroTik Hotspot auto-login.  
Display name: **KIOIS Net** / **KIOIS เน็ตหอ** (binary: `dormnet.exe`).

See also the root repo README: [`../README.md`](../README.md#kiois-เน็ตหอ-kiois-net)

Docs:

- [`docs/architecture.md`](docs/architecture.md) — layers, state machine, probes
- [`docs/login-analysis.md`](docs/login-analysis.md) — live gateway evidence

## Module map (layered)

| Layer | Path | Change when… |
|-------|------|----------------|
| **domain** | `src/domain/` | State enum, `Credentials` type, retry policy |
| **application** | `src/application/` | Runtime loop, `AppConfig`, session monitor / auth gate |
| **infrastructure** | `src/infrastructure/` | HTTP probes, MikroTik auth, keyring, logging, single-instance |
| **presentation** | `src/presentation/` | CLI (`gui` / `run` / `login` / `creds`) and egui GUI |
| entry | `src/main.rs` | Thin: calls `dormnet::presentation::cli::run()` |

```text
presentation → application → domain
       ↘        ↘
        infrastructure (adapters)
```

## Install / uninstall

```powershell
cd DormNet
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1 -LaunchGui
```

Install root: `%LOCALAPPDATA%\DormNet` (unchanged by this restructure).

## Build / test

```powershell
cargo test --manifest-path DormNet/Cargo.toml
cargo build --release --manifest-path DormNet/Cargo.toml
```

## Commands

| Command | Purpose |
|---------|---------|
| `dormnet` / `dormnet gui` | Credential GUI |
| `dormnet run` | Background monitor (autostart) |
| `dormnet login` | One-shot login |
| `dormnet creds set\|clear` | CLI credential manager |
