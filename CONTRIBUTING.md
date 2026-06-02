# Contributing

Echo is under active MVP development. Keep changes focused and run the same checks used by CI before opening a pull request.

## Development

Prerequisites:

- Rust 1.95.0, installed automatically by `rust-toolchain.toml` when using rustup.
- A local Docker-compatible engine for manual app testing.

Useful checks:

```sh
cargo fmt --check
cargo check --locked
cargo test --locked
cargo clippy --locked -- --deny warnings
```

For release validation:

```sh
cargo build --release --locked
```

## Pull Requests

- Include focused tests for behavior changes.
- Keep UI changes consistent with the existing GPUI and gpui-component patterns.
- Do not commit generated build output, local caches, or `scripts/node_modules`.
