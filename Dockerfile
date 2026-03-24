# syntax=docker/dockerfile:1.7

FROM rust:1.88-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /var/lib/santi/runtime /var/lib/santi /root/.cargo /root/target

WORKDIR /app/crates/api

ENV HOME=/root \
    CARGO_HOME=/root/.cargo \
    CARGO_TARGET_DIR=/root/target \
    CARGO_INCREMENTAL=1 \
    BIND_ADDR=0.0.0.0:8080 \
    DATABASE_URL=postgres://santi:santi@postgres:5432/santi?sslmode=disable \
    EXECUTION_ROOT=/app \
    RUNTIME_ROOT=/var/lib/santi/runtime \
    RUST_LOG=santi_api=info

COPY Cargo.toml /app/Cargo.toml
COPY Cargo.lock /app/Cargo.lock
COPY crates/api/Cargo.toml /app/crates/api/Cargo.toml
COPY crates/redis-lock/Cargo.toml /app/crates/redis-lock/Cargo.toml
COPY crates/api/src/lib.rs /app/crates/api/src/lib.rs
COPY crates/api/src/main.rs /app/crates/api/src/main.rs
COPY crates/redis-lock/src/lib.rs /app/crates/redis-lock/src/lib.rs

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo fetch --manifest-path /app/crates/api/Cargo.toml --locked

COPY crates/api /app/crates/api
COPY crates/redis-lock /app/crates/redis-lock

EXPOSE 8080

CMD ["cargo", "run", "--manifest-path", "/app/crates/api/Cargo.toml"]
