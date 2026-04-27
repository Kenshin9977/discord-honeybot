# honeybot

Self-hosted Discord moderation bot focused on the two pieces Discord still
doesn't ship natively in 2026:

1. **Honeypot channels** — any message in a configured channel triggers an
   immediate ban / kick / timeout. AutoMod cannot ban; this can.
2. **Persistent warns + auto-escalation** — `/warn` users, configure
   thresholds (e.g. *3 warns → 1 h timeout, 5 → ban*), the bot enforces them.

Plus a **federated ban-sharing layer** (opt-in pools): when a raider gets
banned on one server, every server subscribed to the same pool can ban them
too — automatically or after a moderator click. No comparable open-source
bot ships this.

> Everything Discord already does well — kick/ban/timeout buttons, AutoMod
> regex, slowmode, audit log — is left to Discord. This bot only fills the
> gaps.

Two binaries:

| crate | role | runtime |
|---|---|---|
| `honeybot` | the Discord bot itself | one container per deployment, SQLite local |
| `honeybot-registry` | federation server (optional) | one shared instance, Postgres |

## Status

⚠️ **Pre-alpha.** v1 is being rewritten from a 60-line Python prototype into
a Rust workspace. Nothing is functional yet. Tracking issues: TBD.

## Quickstart (planned, not yet implemented)

```sh
# self-host the bot only — federation off
docker run -d --restart=unless-stopped \
  -e DISCORD_TOKEN=xxx \
  -v honeybot-data:/data \
  ghcr.io/Kenshin9977/honeybot:latest

# inside Discord, as a server admin:
/honeybot setup
```

To opt into federated bans, add `-e REGISTRY_URL=https://your.registry.example`
and use `/pool join <invite-code>` once the bot is running.

## Documentation

- [docs/setup-bot.md](docs/setup-bot.md) — install paths (Docker, static
  binary, systemd) and first-run wizard.
- [docs/federation.md](docs/federation.md) — pool model, trust, anti-abuse.
- [docs/self-host-registry.md](docs/self-host-registry.md) — running your own
  registry for a private federation.

## Build

Requires Rust ≥ 1.85.

```sh
cargo build --release
```

## License

[AGPL-3.0-or-later](LICENSE). If you fork this and run it as a hosted service,
you must publish your changes.
