# kios

Layered Rust app skeleton (single crate).

```text
src/
  domain/           # pure types — no I/O
  application/      # use-cases / orchestration
  infrastructure/   # adapters
  presentation/     # CLI / UI
  main.rs           # thin → presentation::cli::run()
  lib.rs
```

```powershell
cargo run --manifest-path kios/Cargo.toml
cargo test --manifest-path kios/Cargo.toml
```
