# Self-hosting the registry

> Status: planning document.

You only need to run a registry if you want a **private federation** —
typically a closed group of servers (community network, internal company
servers) that don't want their bans visible on the public instance.

For most people, pointing your bot at the public instance is enough.

## Requirements

- A reachable HTTPS endpoint (a reverse proxy with TLS in front of the
  binary is the simplest setup).
- PostgreSQL ≥ 16.
- A Discord bot token *for the registry itself* — only used to verify that
  guilds claiming registration genuinely host a Discord bot. (The registry
  does not need to be in any server.)

## docker-compose example

```yaml
services:
  registry:
    image: ghcr.io/Kenshin9977/honeybot-registry:latest
    environment:
      DATABASE_URL: postgres://honeybot:secret@db/honeybot
      BIND_ADDR: 0.0.0.0:8080
      DISCORD_VERIFICATION_TOKEN: ${DISCORD_TOKEN}
    depends_on: [db]
    ports: ["8080:8080"]

  db:
    image: postgres:16
    environment:
      POSTGRES_DB: honeybot
      POSTGRES_USER: honeybot
      POSTGRES_PASSWORD: secret
    volumes: [pgdata:/var/lib/postgresql/data]

volumes:
  pgdata:
```

`docker compose run --rm registry honeybot-registry migrate` once before the
first `up` to apply Postgres migrations.

## Pointing bots at your registry

Each `honeybot` deployment that should join your federation just needs:

```
REGISTRY_URL=https://registry.example.com
```

That's it. The bot's first call is `POST /auth/register`; everything else
flows from there.

## Operational notes

- Back up Postgres regularly; ban events are the system of record.
- Watch the `audit` table for anomalies (bursts of registrations from one
  IP, unusually high publish rates).
- Set `RUST_LOG=info` in production; `debug` is verbose and may log
  request bodies.
