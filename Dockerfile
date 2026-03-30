# syntax=docker/dockerfile:1.7

FROM rust:1.88-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        gh \
        git \
        nodejs \
        npm \
    && rm -rf /var/lib/apt/lists/*

RUN npm install -g pnpm

RUN ln -sf /usr/local/cargo/bin/cargo /usr/local/bin/cargo \
    && ln -sf /usr/local/cargo/bin/rustc /usr/local/bin/rustc \
    && ln -sf /usr/local/cargo/bin/rustup /usr/local/bin/rustup

RUN mkdir -p /var/lib/santi/runtime /var/lib/santi /root/.cargo /root/target

WORKDIR /app/crates/santi-api

ENV HOME=/root \
    PATH=/usr/local/cargo/bin:${PATH} \
    CARGO_HOME=/root/.cargo \
    CARGO_TARGET_DIR=/root/target \
    CARGO_INCREMENTAL=1 \
    BIND_ADDR=0.0.0.0:8080 \
    SANTI_BASE_URL=http://127.0.0.1:8080 \
    DATABASE_URL=postgres://santi:santi@postgres:5432/santi?sslmode=disable \
    EXECUTION_ROOT=/app \
    RUNTIME_ROOT=/var/lib/santi/runtime \
    RUST_LOG=santi_api=info

COPY Cargo.toml /app/Cargo.toml
COPY Cargo.lock /app/Cargo.lock
COPY crates/santi-core/Cargo.toml /app/crates/santi-core/Cargo.toml
COPY crates/santi-db/Cargo.toml /app/crates/santi-db/Cargo.toml
COPY crates/santi-provider/Cargo.toml /app/crates/santi-provider/Cargo.toml
COPY crates/santi-runtime/Cargo.toml /app/crates/santi-runtime/Cargo.toml
COPY crates/santi-api/Cargo.toml /app/crates/santi-api/Cargo.toml
COPY crates/santi-cli/Cargo.toml /app/crates/santi-cli/Cargo.toml
COPY crates/santi-lock/Cargo.toml /app/crates/santi-lock/Cargo.toml
COPY crates/santi-core/src/lib.rs /app/crates/santi-core/src/lib.rs
COPY crates/santi-db/src/lib.rs /app/crates/santi-db/src/lib.rs
COPY crates/santi-provider/src/lib.rs /app/crates/santi-provider/src/lib.rs
COPY crates/santi-runtime/src/lib.rs /app/crates/santi-runtime/src/lib.rs
COPY crates/santi-api/src/lib.rs /app/crates/santi-api/src/lib.rs
COPY crates/santi-api/src/main.rs /app/crates/santi-api/src/main.rs
COPY crates/santi-cli/src/main.rs /app/crates/santi-cli/src/main.rs
COPY crates/santi-lock/src/lib.rs /app/crates/santi-lock/src/lib.rs

RUN --mount=type=cache,target=/root/.cargo/registry \
    --mount=type=cache,target=/root/.cargo/git \
    cargo fetch --manifest-path /app/crates/santi-api/Cargo.toml --locked

COPY crates/santi-api /app/crates/santi-api
COPY crates/santi-cli /app/crates/santi-cli
COPY crates/santi-core /app/crates/santi-core
COPY crates/santi-db /app/crates/santi-db
COPY crates/santi-lock /app/crates/santi-lock
COPY crates/santi-provider /app/crates/santi-provider
COPY crates/santi-runtime /app/crates/santi-runtime

EXPOSE 8080

CMD ["cargo", "run", "--manifest-path", "/app/crates/santi-api/Cargo.toml"]
