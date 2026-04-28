#!/usr/bin/env bash
# scripts/init.sh — interactive first-time setup. Walks the operator
# through getting a Discord bot token and writes it to .env.
#
# Idempotency: refuses to overwrite an existing .env. Remove it first if
# you want to redo the wizard.

set -euo pipefail

if [ -f .env ]; then
    echo ".env already exists."
    echo "Edit it directly, or 'rm .env && make init' to start over."
    exit 0
fi

cat <<'BANNER'
─────────────────────────────────────────────────────────────────────────
 honeybot — first-time setup
─────────────────────────────────────────────────────────────────────────

You only need a Discord bot token.

  1. Open: https://discord.com/developers/applications
  2. Click 'New Application' (or pick an existing one)
  3. Open the 'Bot' tab on the left
  4. Click 'Reset Token' and copy the value

(No 'privileged intents' to enable — honeybot uses only the basic ones.)

─────────────────────────────────────────────────────────────────────────

BANNER

# `-s` hides the typed characters; tokens are sensitive.
read -r -s -p "Paste your Discord bot token: " token
echo
echo

# Strip whitespace and refuse blanks; pasting from a browser sometimes
# adds a trailing newline or stray space.
token="${token//[[:space:]]/}"
if [ -z "$token" ]; then
    echo "Empty token — aborting." >&2
    exit 1
fi

# 077 → owner read/write only. The token is a credential.
umask 077
{
    echo "DISCORD_TOKEN=$token"
    echo "RUST_LOG=info"
} > .env

echo "Wrote .env (git-ignored, mode 600)."
echo
echo "Next: 'make run' to start the bot."
echo "      It will print an invite URL — click it to add the bot to a server."
