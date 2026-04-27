# honeybot

Self-hosted Discord moderation bot focused on the two pieces Discord still
doesn't ship natively in 2026:

1. **Honeypot channels** — any message in a configured channel triggers an
   immediate ban / kick / timeout. AutoMod cannot ban; this can.
2. **Persistent warns + auto-escalation** — `/warn` users, configure
   thresholds (e.g. *3 warns → 1 h timeout, 5 → ban*), the bot enforces them.

> Everything Discord already does well — kick/ban/timeout buttons, AutoMod
> regex, slowmode, audit log — is left to Discord. This bot only fills the
> gaps.

Single binary, single SQLite file, single container. Multi-guild,
multi-language (FR/EN), configured entirely via slash commands after the
first launch.

## Status

⚠️ **Pre-alpha.** v1 is being rewritten from a 60-line Python prototype.
Code compiles and is internally consistent but has not yet been smoke-tested
against a live Discord server.

A federated cross-server ban-sharing layer was scoped for v1 but cut after
threat-modeling exposed several poisoning vectors (malicious publishers
mass-banning legitimate users on every subscriber, public-pool admission
abuse, etc.). It is deferred to v2 with a security review before any code
ships.

## Quickstart (planned)

```sh
docker run -d --restart=unless-stopped \
  -e DISCORD_TOKEN=xxx \
  -v honeybot-data:/data \
  ghcr.io/Kenshin9977/honeybot:latest
```

Inside Discord, as a server admin:

```
/honeybot setup           # interactive first-run wizard
/honeypot add #trap ban
/warn thresholds set 3 timeout 60
```

## Documentation

[docs/setup-bot.md](docs/setup-bot.md) — install paths (Docker, static
binary, systemd) and first-run wizard.

## Build

Requires Rust ≥ 1.85.

```sh
cargo build --release
```

## License

[AGPL-3.0-or-later](LICENSE). If you fork this and run it as a hosted
service, you must publish your changes.
