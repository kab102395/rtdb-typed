# Contributing

Run the deterministic checks before opening a change:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo check --examples
```

For end-to-end local Firebase coverage, run `./scripts/test-emulator.sh`.
It uses only the `demo-rtdb-typed` emulator project, prints Docker port
bindings, and refuses occupied ports. Never use the open emulator rules in a
production Firebase project and never commit credentials.
