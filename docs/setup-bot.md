# Setting up the bot

> Status: planning document. Commands shown reflect the v1 target, not the
> current code.

## Prerequisites

1. A Discord application + bot user. Create one at
   <https://discord.com/developers/applications>. **No privileged intents
   need to be enabled** — the bot uses only `Guilds` and `Guild Messages`.
2. A bot token (`Bot` tab → *Reset Token*). Treat it like a password.
3. A target server where you have `MANAGE_GUILD`. The bot prints an invite
   URL with the right permissions pre-set on its first startup; no manual
   OAuth URL Generator step needed.

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

The bot prints its invite URL on startup. Open it, pick a server. Once the
bot is online, any admin with `MANAGE_GUILD` runs:

```
/honeybot setup
```

That's it — defaults to English with the current channel as the
notification target. Override either with the `language:` and
`notification_channel:` options. All later changes are also slash-driven;
you should never need to edit `.env` again.
