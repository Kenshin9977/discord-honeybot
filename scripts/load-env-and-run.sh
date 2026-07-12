#!/usr/bin/env bash
# scripts/load-env-and-run.sh — load .env, validate DISCORD_TOKEN, then
# `cargo run`. If the token is missing or empty, dump a diagnostic of
# what was actually found in .env (with the token masked).

set -euo pipefail

if [ ! -f .env ]; then
    echo "no .env in $(pwd). Run 'make init' first." >&2
    exit 1
fi

# Strip a possible UTF-8 BOM from the first line. Some editors save .env
# files with a BOM, which makes `source` see `﻿DISCORD_TOKEN` as the
# variable name and silently fail to set the right one.
if head -c 3 .env | LC_ALL=C grep -q $'\xef\xbb\xbf'; then
    tmp="$(mktemp)"
    tail -c +4 .env > "$tmp"
    mv "$tmp" .env
    chmod 600 .env
    echo "fixed: stripped UTF-8 BOM from .env"
fi

# `set -a` exports every variable defined in `source`d files automatically.
set -a
# shellcheck disable=SC1091
source .env
set +a

if [ -z "${DISCORD_TOKEN:-}" ]; then
    echo
    echo "DISCORD_TOKEN is missing or empty after loading .env."
    echo
    echo "What .env actually contains (values masked):"
    echo "─────────────────────────────────────────────"
    awk -F= '
        /^[[:space:]]*$/ || /^[[:space:]]*#/ { next }
        {
            key = $1
            sub(/^[[:space:]]+/, "", key)
            sub(/[[:space:]]+$/, "", key)
            printf "%s=***\n", key
        }
    ' .env
    echo "─────────────────────────────────────────────"
    echo
    echo "Common causes:"
    echo "  • Spaces around the '=' sign. It must be 'DISCORD_TOKEN=abc',"
    echo "    not 'DISCORD_TOKEN = abc'. Fix .env and retry."
    echo "  • The token line was accidentally commented out (starts with #)."
    echo "  • Quick reset: 'rm .env && make init' to redo the wizard."
    echo
    exit 1
fi

exec cargo run
