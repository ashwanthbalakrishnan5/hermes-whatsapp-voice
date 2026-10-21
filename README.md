# hermes-whatsapp-voice

Live voice calls with [Hermes](https://github.com/NousResearch/hermes-agent) over WhatsApp.

- **Call your bot.** Call your Hermes bot's WhatsApp number and talk to it. The voice agent can hand
  real work to Hermes mid-call ("move my 3pm to tomorrow", "what did Sam say about the invoice?").
- **Let Hermes call you.** For things that matter, Hermes can ring you instead of sending a message:
  a top-priority reminder, an urgent alert, a cron job that needs a decision. If you don't pick up,
  the tool says so and Hermes falls back to another channel.
- **Let Hermes call other people for you** (opt-in). Hermes can phone the people in your address
  book on your behalf, hold the conversation in their language, and report back. See
  [Calling other people](#calling-other-people-opt-in).
- **Summary afterwards.** When a call ends, Hermes digests the transcript and sends you a short
  summary in WhatsApp.

It works with the personal-number setup most Hermes users already have (WhatsApp linked as a
device). No WhatsApp Business / Cloud API account is needed.

## How it works

```
 your phone ──WhatsApp call──▶ bot number ──▶ hermes-wa-voice (linked device, Rust)
                                                 │  call audio (16 kHz in, 24 kHz out, resampled)
                                                 ▼
                                            Gemini Live (speech-to-speech)
                                                 │  ask_hermes(...) tool calls
                                                 ▼
                                            Hermes API server (/v1/runs)
```

- **`hermes-wa-voice`** is a small sidecar built on
  [whatsapp-rust](https://github.com/oxidezap/whatsapp-rust). It links to the bot's WhatsApp number
  as one more linked device (WhatsApp allows four), answers calls from your number only, and places
  calls to your number only.
- Call audio is bridged to **Gemini Live** (`gemini-3.8-live` by default) for natural, interruptible
  speech-to-speech.
- The voice model gets two tools:
  - `ask_hermes`, which runs a Hermes turn through the API server and speaks the result when it
    arrives;
  - `end_call`, which hangs up after a goodbye.
- The **Hermes plugin** adds two agent tools: `whatsapp_call_owner(reason, details?)` and
  `whatsapp_call_status(call_id?)`. It also adds a `hermes whatsapp-voice` command for setup.

## Requirements

- Hermes `>= 0.21.5` on Linux or macOS.
- A separate WhatsApp number for the bot: Hermes's WhatsApp `bot` mode, or any spare number. In
  `self-chat` mode the bot is your own number, and you can't call yourself.
- Gemini set up in Hermes with an API key. The plugin reuses the key of Hermes's Gemini provider
  (`GOOGLE_API_KEY`, else `GEMINI_API_KEY`), so there is nothing extra to enter. Without one,
  `hermes model` (choose Gemini) adds it. Google login (Cloud Code OAuth) does not work for the
  Live API. The live models have a free tier, but Google may use free-tier data. Paid use is
  roughly $0.005/min of audio in and $0.018/min out.
- Optional, but needed for the voice agent to reach Hermes: the Hermes
  [API server](https://hermes-agent.nousresearch.com/docs/user-guide/features/api-server) enabled.
  It must have an `API_SERVER_KEY`.

## Install

**Let your agent do it.** Tell Hermes (or any coding agent on that machine) to follow
[skills/setup/SKILL.md](skills/setup/SKILL.md). It's a step-by-step runbook written for agents,
and it stops to ask you only for your number and to scan a QR code. After installing, Hermes can
also load it as `hermes-whatsapp-voice:setup`.

**Or by hand:**

```bash
hermes plugins install ashwanthbalakrishnan5/hermes-whatsapp-voice --enable
```

### 1. Set your number

This is the only caller that will be answered, and the only number called unless you turn on
[calls to other people](#calling-other-people-opt-in).

```bash
hermes config set plugins.entries.hermes-whatsapp-voice.settings.owner_number +15551234567
```

### 2. Link the sidecar to the bot's WhatsApp number (once)

Easiest: open the Hermes dashboard, go to the **WhatsApp Voice** tab, click **Pair with
WhatsApp**, and scan the QR from the **bot's** phone (WhatsApp > Linked devices > Link a device).
From a terminal instead:

```bash
hermes whatsapp-voice pair                        # shows a QR code
hermes whatsapp-voice pair --phone +15550001111   # the BOT's number; prints a pair code
```

The first run also fetches the sidecar binary for your platform from this repo's GitHub release.
You don't need Rust or any other toolchain. The download is checked against checksums pinned in
this repo (`sidecar/release-checksums.txt`). If the GitHub CLI is installed, its GitHub build attestation is checked too, which
proves it was built from this repository by its release workflow. Prebuilt binaries cover linux
amd64/arm64 and macOS arm64. To build it yourself instead (Rust 1.94+, cmake, a C compiler), run
`hermes whatsapp-voice install --from-source`.

> **Why a separate link when Hermes is already logged in to WhatsApp?** WhatsApp lets each linked
> device keep only one live connection. Sharing Hermes's WhatsApp session would make the two
> connections knock each other offline, and Hermes's messaging would break. Only the bot's phone
> can approve a new device, so the link happens once. It uses one of WhatsApp's four linked-device
> slots.

### 3. Enable the Hermes API server (recommended)

```bash
# in Hermes's .env (~/.hermes/.env by default)
API_SERVER_ENABLED=true
API_SERVER_KEY=<a long random string>
```

Restart the gateway after this.

### 4. Run the sidecar

```bash
hermes whatsapp-voice serve      # foreground, to try it out
hermes whatsapp-voice service    # prints a systemd user unit (Linux) or launchd plist (macOS)
```

To keep it running, save the printed unit and enable it as the comments in it describe. Check it
with:

```bash
hermes whatsapp-voice status
hermes whatsapp-voice call "Testing the new voice line"
```

## Using it

Call your bot's number from your phone. Or ask Hermes to call you:

> Call me at 7:30 tomorrow about the flight check-in, and if I don't answer, set an urgent reminder.

`whatsapp_call_owner` works from cron runs too, so a scheduled job can escalate to a phone call:

```bash
hermes cron create "0 9 * * 1-5" \
  "If anything on my calendar today is marked urgent, call me about it with whatsapp_call_owner."
```

The tool waits until the call is answered, declined or unanswered, and returns that status. On
`no_answer`/`rejected`/`failed` it tells the agent to use another channel.

## Calling other people (opt-in)

With `contact_calls` on, your agent becomes a real phone assistant. You tell Hermes what you need in
chat, and it calls the right person, has the conversation, and sends you the answer:

- **You're in a meeting and someone calls.** You can't pick up, so you text Hermes: *"Call Sam back,
  tell him I'm in a meeting and I'll call him after 4."* Hermes calls Sam, passes on the message,
  asks if it's urgent, and tells you what he said.
- **Order dinner.** *"Call the pizza place, order a large margherita for 8 pm, and ask how long it
  takes."*
- **Organise a meeting.** *"Call Alex and Priya and find a time on Thursday afternoon that works for
  both."* Hermes calls them one after the other and reports the slot that fits.
- **Keep family in the loop.** *"Call mom, tell her the flight lands at 6, and ask what she'd like
  for dinner."* Each contact can have their own language (say `ru-RU` for a Russian-speaking
  relative), so Hermes speaks to them in theirs and reports back to you in yours.
- **Quick checks.** *"Call the garage and ask if the car is ready."*

How it behaves on the call:

- It says up front that it is an AI assistant calling on your behalf, and never pretends to be human.
- It goes through the task one step at a time: one message or one question per turn, waits for the
  answer, and comes back once to a question that was skipped.
- It only does the task you gave. On these calls it has **no** `ask_hermes`, no access to your memory,
  files or tools, and it won't share private details beyond what the task needs. Anything the other
  person asks for is passed on to you instead.
- If the line is busy with another call (you're talking to Hermes yourself, say), the call is queued
  and placed as soon as the line is free.
- Afterwards you get a WhatsApp report: did it reach them, and what exactly they answered.

Safety rails, all configurable:

| Setting | Default | What it does |
|---|---|---|
| `contact_calls` | `false` | The whole feature. Off: the tool is hidden and nobody but you is ever called. |
| `allow_unsaved_numbers` | `false` | Off: only saved contacts can be called, so a prompt injection can't make the agent dial an arbitrary number. |
| `confirm_contact_calls` | `true` | The agent must ask you on WhatsApp and get a yes before each call. |
| `max_calls_per_contact_per_day` | `3` | Rate limit per person. |

Requests coming from group chats are refused. Contacts are saved from your private chat ("save
mom as +44 7700 900123, she speaks English") with `whatsapp_voice_contacts`, or from the CLI:

```bash
hermes whatsapp-voice call "Tell Sam I'm running 10 minutes late" --to +15551234567 --name Sam
```

Before you turn this on, check the rules for AI calls and call recording in your country and the
callee's. A number your contact has not saved may be silenced by WhatsApp's "Silence unknown
callers" setting, so ask the people you call often to save the bot's number.

## Configure

Everything is configurable from the **WhatsApp Voice** tab in the Hermes dashboard, the Desktop
app's plugin settings, or `hermes config set`. That covers the voice, the model, your own
instructions (language, tone, call length), and any Gemini Live option via `live_config`. The
voice takes its name and personality from your own Hermes. Full reference:
[docs/configuration.md](docs/configuration.md).

## What this plugin does that you should know about

- **Long-running process.** The sidecar runs as a separate process that you start (`serve`, or the
  systemd/launchd unit). The plugin itself only talks to it over loopback HTTP, authenticated with a
  random token stored in the plugin's data directory.
- **Stored credentials.** The sidecar's WhatsApp linked-device session is stored at
  `<hermes home>/plugin-data/hermes-whatsapp-voice/whatsapp.db` (the folder is private, mode 700).
  Anyone with that file can act as that linked device. Unlink it any time from the bot's phone
  (Linked devices).
- **Network calls.**
  - WhatsApp servers (signaling and call media relays).
  - `generativelanguage.googleapis.com` (Gemini), using the Gemini key Hermes already has. It
    receives call audio during calls, plus, as text: the persona briefing (Hermes's name,
    personality and what it knows about you), the reason and details of an outgoing call, and the
    results of `ask_hermes` requests.
  - Your local Hermes API server, using `API_SERVER_KEY`, for `ask_hermes` (which runs a full
    Hermes turn with your tools), the persona briefing (fetched at startup and after each call),
    and the post-call summary. That summary run asks Hermes to note anything worth remembering, so
    it can write to Hermes's memory.
  - `github.com`, to download the sidecar binary on first `pair`/`serve`/`install` (or from the
    dashboard's Pair button).

  Nothing else, and no telemetry.
- **Who it talks to.** It only answers calls from `owner_number`. By default it only calls
  `owner_number`: `whatsapp_call_owner` takes no destination parameter. Calls to anyone else need
  `contact_calls` turned on, and are limited to saved contacts, confirmed by you and rate limited
  unless you change those settings (see [Calling other people](#calling-other-people-opt-in)).
  Calls from other numbers are left alone, so the bot's other devices can still take them.
- **Commands it runs.**
  - The dashboard tab's backend (inside the Hermes dashboard, behind its login): `hermes config
    set` for this plugin's settings (only keys declared in `plugin.yaml`); the sidecar's `pair`;
    and `systemctl --user restart` or `launchctl kickstart` for the sidecar service.
  - The CLI: `gh attestation verify` when the GitHub CLI is logged in, and `cargo build` for
    `install --from-source`.
  - The setup skill (only when an agent follows it) appends `API_SERVER_ENABLED` and a generated
    `API_SERVER_KEY` to Hermes's `.env` if they're missing.
- **Secrets** are read through Hermes (`get_secret`) and passed to the sidecar in its environment.
  The plugin never reads `.env` itself. The sidecar's local API only binds to a loopback address.

## Caveats

- WhatsApp calling here uses an **unofficial**, reverse-engineered protocol (via whatsapp-rust).
  WhatsApp updates can break it, and automated use may violate WhatsApp's Terms of Service. Use a
  number you can afford to lose.
- One call at a time. Group calls and video are not supported.
- A Gemini Live connection lasts about 10 minutes. The sidecar resumes the session transparently, but
  a resume can fail and end a long call.

## Development

```bash
python -m pip install -r requirements-dev.txt
ruff check . && python -m pytest tests/ -q
cd sidecar && cargo test
GEMINI_API_KEY=... cargo test -- --ignored   # live round trip against Gemini
```

Layout: the repo root is the Hermes plugin (`plugin.yaml`, `__init__.py`, `tools.py`, `cli.py`,
`sidecar_client.py`), and `sidecar/` is the Rust sidecar. Tagging `vX.Y.Z` builds attested release
binaries (`.github/workflows/release.yml`). Then commit the release's `SHA256SUMS` as
`sidecar/release-checksums.txt`. Keep `version` in `plugin.yaml` and `sidecar/Cargo.toml` in step
with the tag.

## Credits

- [whatsapp-rust](https://github.com/oxidezap/whatsapp-rust) (MIT) does the WhatsApp protocol and the
  call media work.
- Gemini Live provides the voice.

## License

MIT
