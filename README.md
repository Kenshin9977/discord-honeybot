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

## Quickstart — three steps

### 1. Get a Discord bot token

[discord.com/developers/applications](https://discord.com/developers/applications)
→ **New Application** → **Bot** tab → **Reset Token** → copy.

That's all you need to touch on this page. Specifically:

- **Privileged Gateway Intents** stay **off**. (Most bots ask you to
  enable 'Server Members' and 'Message Content'; honeybot doesn't read
  message text or subscribe to member events, so the privileged intents
  are unused.)
- **No OAuth URL Generator step**. The bot prints its own invite URL with
  the right server permissions (Ban / Kick / Moderate Members, etc.)
  pre-baked when it starts up.

### 2. Run it

```sh
make run
```

First time, the wizard asks for the token, writes a git-ignored `.env`,
then starts the bot. Subsequent runs reuse it. The startup log includes:

```
invite URL — open it to add this bot to a server
url=https://discord.com/oauth2/authorize?client_id=…&scope=bot+applications.commands&permissions=…
```

Click it, pick a server, authorise.

### 3. Configure inside Discord

In any channel where you want notifications, type:

```
/honeybot setup
```

That's it — defaults to English and uses the current channel for trigger
notifications. Then add a honeypot:

```
/honeypot add channel:#trap action:Ban
```

Anyone posting in `#trap` is now banned and DMed. Test it from a second
account.

## Other commands

```
/honeybot lang en|fr                          change locale later
/honeybot notif #channel                       move the notification channel
/honeypot remove channel:#trap                 unconfigure
/honeypot list                                 list honeypots
/honeypot whitelist add #trap @role            exempt a role
/warn add @user reason
/warn list @user
/warn remove <id>
/warn thresholds set 3 timeout 60              after 3 warns → 60 min timeout
/warn thresholds list
```

## Local development

```sh
make test          # 20 unit + handler-level tests, zero Discord token needed
make ci            # mirrors GitHub Actions: fmt + clippy -D warnings + test
make docker        # build & run the docker stack
make help          # all targets
```

The handler tests drive the real `on_message` path against an in-memory
SQLite and a recording mock that captures every Discord call — no live
gateway, no live HTTP, deterministic.

## Status

⚠️ **Pre-alpha.** v1 was rewritten from a 60-line Python prototype into a
Rust workspace and is internally consistent (CI green, 20 tests passing),
but has not yet been smoke-tested against a live Discord server.

A federated cross-server ban-sharing layer was scoped for v1 but cut after
threat-modeling exposed several poisoning vectors (malicious publishers
mass-banning legitimate users on every subscriber, public-pool admission
abuse, etc.). It is deferred to v2 with a security review before any code
ships.

## Documentation

[docs/setup-bot.md](docs/setup-bot.md) — alternate install paths (static
binary, systemd) for users who don't want Docker.

## License

[AGPL-3.0-or-later](LICENSE). If you fork this and run it as a hosted
service, you must publish your changes.
