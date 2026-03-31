# syntax=docker/dockerfile:1.7

FROM ghcr.io/perishcode/docker/santi-builder:v1 AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates/santi-core/Cargo.toml crates/santi-core/Cargo.toml
COPY crates/santi-api/Cargo.toml crates/santi-api/Cargo.toml
COPY crates/santi-cli/Cargo.toml crates/santi-cli/Cargo.toml
COPY crates/santi-db/Cargo.toml crates/santi-db/Cargo.toml
COPY crates/santi-ebus/Cargo.toml crates/santi-ebus/Cargo.toml
COPY crates/santi-lock/Cargo.toml crates/santi-lock/Cargo.toml
COPY crates/santi-provider/Cargo.toml crates/santi-provider/Cargo.toml
COPY crates/santi-runtime/Cargo.toml crates/santi-runtime/Cargo.toml

RUN mkdir -p \
    crates/santi-core/src \
    crates/santi-api/src \
    crates/santi-cli/src \
    crates/santi-db/src \
    crates/santi-ebus/src \
    crates/santi-lock/src \
    crates/santi-provider/src \
    crates/santi-runtime/src

COPY crates/santi-core/src/lib.rs crates/santi-core/src/lib.rs
COPY crates/santi-api/src/lib.rs crates/santi-api/src/lib.rs
COPY crates/santi-api/src/main.rs crates/santi-api/src/main.rs
COPY crates/santi-cli/src/main.rs crates/santi-cli/src/main.rs
COPY crates/santi-db/src/lib.rs crates/santi-db/src/lib.rs
COPY crates/santi-ebus/src/lib.rs crates/santi-ebus/src/lib.rs
COPY crates/santi-lock/src/lib.rs crates/santi-lock/src/lib.rs
COPY crates/santi-provider/src/lib.rs crates/santi-provider/src/lib.rs
COPY crates/santi-runtime/src/lib.rs crates/santi-runtime/src/lib.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo fetch --manifest-path crates/santi-api/Cargo.toml --locked

COPY crates ./crates

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/tmp/cargo-target \
    cargo build --locked --profile container-dev -p santi-api \
    && mkdir -p /opt/artifacts \
    && cp /tmp/cargo-target/container-dev/santi-api /opt/artifacts/santi-api

FROM ghcr.io/perishcode/docker/santi-runtime:v1 AS runtime

ENV HOME=/root \
    BIND_ADDR=0.0.0.0:8080 \
    SANTI_BASE_URL=http://127.0.0.1:8080 \
    DATABASE_URL=postgres://santi:santi@postgres:5432/santi?sslmode=disable \
    EXECUTION_ROOT=/app \
    RUNTIME_ROOT=/runtime \
    RUST_LOG=santi_api=info

COPY --from=builder /opt/artifacts/santi-api /usr/local/bin/santi-api

EXPOSE 8080

CMD ["santi-api"]
