# Setting up the bot

> Status: planning document. Commands shown reflect the v1 target, not the
> current code.

## Prerequisites

1. A Discord application + bot user. Create one at
   <https://discord.com/developers/applications>. Enable the **Server Members**
   and **Message Content** privileged intents.
2. A bot token (`Bot` tab → *Reset Token*). Treat it like a password.
3. A target server where you have `MANAGE_GUILD`. Use the OAuth2 URL generator
   to invite the bot with at least these permissions:
   *Manage Messages*, *Ban Members*, *Kick Members*, *Moderate Members*,
   *Read Message History*, *Send Messages*, *Embed Links*.

## Path 1 — Docker (recommended)

```sh
docker run -d --name honeybot --restart=unless-stopped \
  -e DISCORD_TOKEN=YOUR_TOKEN \
  -v honeybot-data:/data \
  ghcr.io/Kenshin9977/honeybot:latest
```

## Path 2 — Static binary

```sh
curl -L https://github.com/Kenshin9977/discord-honeybot/releases/latest/download/honeybot-linux-arm64-musl \
  -o honeybot
chmod +x honeybot
DISCORD_TOKEN=YOUR_TOKEN ./honeybot serve
```

Pre-built binaries: `linux-{amd64,arm64}`, `darwin-arm64`, `windows-amd64`.

## Path 3 — systemd

Sample unit file (place in `/etc/systemd/system/honeybot.service`):

```ini
[Unit]
Description=honeybot Discord moderation bot
After=network-online.target

[Service]
Type=simple
User=honeybot
EnvironmentFile=/etc/honeybot/env
ExecStart=/usr/local/bin/honeybot serve
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

`/etc/honeybot/env`:

```
DISCORD_TOKEN=YOUR_TOKEN
DATABASE_URL=sqlite:///var/lib/honeybot/honeybot.db?mode=rwc
```

## First-run wizard

Once the bot is online in your server, any user with `MANAGE_GUILD` runs:

```
/honeybot setup
```

The wizard prompts for: locale (`en` / `fr`), notification channel,
optional first honeypot channel and action. All later changes are also
slash-driven — you should never need to edit `.env` again.
