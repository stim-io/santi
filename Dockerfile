# syntax=docker/dockerfile:1.7

FROM rust:1.88-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /var/lib/santi/runtime /var/lib/santi /root/.cargo /root/target

WORKDIR /app/api

ENV HOME=/root \
    CARGO_HOME=/root/.cargo \
    CARGO_TARGET_DIR=/root/target \
    CARGO_INCREMENTAL=1 \
    BIND_ADDR=0.0.0.0:8080 \
    DATABASE_URL=postgres://santi:santi@postgres:5432/santi?sslmode=disable \
    EXECUTION_ROOT=/app \
    RUNTIME_ROOT=/var/lib/santi/runtime \
    RUST_LOG=santi_api=info

COPY api/Cargo.toml /app/api/Cargo.toml
COPY api/Cargo.lock /app/api/Cargo.lock
COPY redis-lock/Cargo.toml /app/redis-lock/Cargo.toml
COPY api/src/lib.rs /app/api/src/lib.rs
COPY api/src/main.rs /app/api/src/main.rs
COPY redis-lock/src/lib.rs /app/redis-lock/src/lib.rs

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo fetch --manifest-path /app/api/Cargo.toml --locked

COPY api/src /app/api/src
COPY api/tests /app/api/tests
COPY redis-lock /app/redis-lock

EXPOSE 8080

CMD ["cargo", "run"]
