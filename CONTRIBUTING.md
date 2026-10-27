# Contributing

Thanks for helping. Bug reports, fixes and features are all welcome.

## Before you start

- **Small fixes** (a bug, a typo, a missing config knob): open the PR directly.
- **Anything bigger** (a new voice backend, a new call flow, a new setting): open an issue or comment
  on an existing one first, so we can agree on the shape before you spend time on it. Several
  areas already have work in flight, and two PRs for the same thing means one gets closed.

## Development

```bash
python -m pip install -r requirements-dev.txt
ruff check . && ruff format --check . && python -m pytest tests/ -q

cd sidecar
cargo fmt --check && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked
GEMINI_API_KEY=... cargo test -- --ignored   # optional: live round trip against Gemini
```

CI runs exactly these checks and must pass before a PR can merge. The README's
[Development](README.md#development) section describes the layout.

## Rules for changes

- **The owner is the only call target.** Call tools take no destination parameter. A tool that
  lets the model choose who to call is a prompt-injection hole.
- **No personal data** in code, tests, fixtures, commit messages or logs pasted into issues: no
  phone numbers, names or persona details. Use obviously fake values (`15550000000`).
- **No committed binaries.** The release workflow builds and attests them.
- **Secrets** are read only through Hermes's `get_secret`, never from files the plugin writes.
- **Disclose behaviour.** If a change adds network access, a new process or credential handling,
  update [What this plugin does](README.md#what-this-plugin-does-that-you-should-know-about).
- **Setting names**: never `model`, `plugins`, `security` or `settings` (Hermes reserves them; a
  test checks this). Document new settings in [docs/configuration.md](docs/configuration.md).

## Pull requests

- One change per PR. PRs are squash-merged, so the PR title becomes the commit message: write it
  as a short, plain description of the change.
- Link the issue it fixes (`Fixes #123`).
- Say how you tested it, especially for anything that touches live calls (which phone client,
  inbound or outbound).

Releases are cut by the maintainer.
