# Security policy

## Reporting a vulnerability

Please **do not open a public issue**. Report it privately through
[GitHub's private vulnerability reporting](https://github.com/ashwanthbalakrishnan5/hermes-whatsapp-voice/security/advisories/new).
You should get a reply within a week.

Useful reports include the version, what an attacker can do, and steps to reproduce.

## In scope

- Anyone other than the owner getting the bot to place a call, run a Hermes tool, or act with the
  owner's session (including through prompt injection during a call).
- The sidecar's loopback API or its bearer token being reachable or guessable.
- Leaks of the WhatsApp session, API keys or call transcripts.
- Release binaries that don't match their attestation.

WhatsApp's own terms of service and the reverse-engineered protocol itself (see the README's
Caveats) are out of scope.

## Supported versions

Only the latest release gets security fixes.
