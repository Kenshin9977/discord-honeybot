# syntax=docker/dockerfile:1.7
FROM lukemathwalker/cargo-chef:latest-rust-1.95-slim AS chef
WORKDIR /app

# ── Plan ─────────────────────────────────────────────────────────────────────
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ── Build ────────────────────────────────────────────────────────────────────
FROM chef AS builder
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config \
 && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json --bin honeybot
COPY . .
RUN cargo build --release --bin honeybot

# ── Runtime ──────────────────────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/honeybot /usr/local/bin/honeybot

USER 65532:65532
WORKDIR /data
VOLUME ["/data"]

ENV DATABASE_URL=sqlite:///data/honeybot.db?mode=rwc \
    RUST_LOG=info

ENTRYPOINT ["/usr/local/bin/honeybot"]
CMD ["serve"]
