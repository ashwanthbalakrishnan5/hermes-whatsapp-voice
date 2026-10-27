## What and why

<!-- What this changes and why. Link the issue: Fixes #123 -->

## How it was tested

<!-- Commands run; for call changes: phone client (iPhone/Android/Desktop) and direction (inbound/outbound). -->

## Checklist

- [ ] `ruff check . && ruff format --check . && python -m pytest tests/ -q` passes
- [ ] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` passes (if `sidecar/` changed)
- [ ] No phone numbers, names or keys in code, tests or commits
- [ ] New settings are documented in `docs/configuration.md`; new network/process behaviour in the README
