## What

`HERMES_TIMEOUT` was the only limit that could not be configured:

```rust
const HERMES_TIMEOUT: Duration = Duration::from_secs(180);   // sidecar/src/calls.rs:26
```

Every sibling limit already has an `HWV_*` knob — `HWV_RING_TIMEOUT`, `HWV_MAX_DURATION` (15m), `HWV_SILENCE_TIMEOUT` — so the one limit that decides **how long Hermes may work on the owner's request** was the only one fixed at compile time. Three minutes is also well under `HWV_MAX_DURATION`, so a call can legitimately outlive its own tool budget.

This adds `hermes_timeout` to `Config`, read from `HWV_HERMES_TIMEOUT` (default `180s`, so nothing changes for anyone who does not set it), and uses it at all three run sites:

- the owner's request during a call (`spawn_ask_hermes`)
- the post-call digest (`send_summary`)
- the persona briefing (`refresh_persona`)

## Why

When the deadline trips, `Hermes::run` bails with `"Hermes did not finish within 180s"` (hermes.rs:112-114) and `StopOnDrop` then POSTs `/v1/runs/{id}/stop`, so work that legitimately needs four minutes — a test run, a repo audit, a long report — is not merely unanswered, it is **cancelled and thrown away**, while the call itself may have 15 minutes to run.

Related issue: #20.

## Testing

- `cargo fmt --check` — clean
- `cargo clippy --locked --all-targets -- -D warnings` — clean
- `cargo test --locked` — 22 passed, 1 ignored (unchanged)

Manual check on a live sidecar: `HWV_HERMES_TIMEOUT=600s` reaches `Config::hermes_timeout` (verified by reading the spawned process env and a run that previously died at 180s completing after ~4 minutes).

I did not touch the drop-guard itself — `StopOnDrop` still cancels work when the owner hangs up. Parking a run and delivering it later is a separate, larger change; happy to open an issue for it if that's welcome.
