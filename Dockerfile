# syntax=docker/dockerfile:1.7

FROM ghcr.io/perishcode/docker/santi-builder:v1 AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY --from=stim_proto_context . /stim-proto

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo fetch --manifest-path crates/santi-api/Cargo.toml --locked

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/tmp/cargo-target \
    rm -rf /tmp/cargo-target/container-dev /tmp/cargo-target/debug /tmp/cargo-target/release \
    && \
    cargo build --locked --profile container-dev -p santi-api \
    && mkdir -p /opt/artifacts \
    && cp /tmp/cargo-target/container-dev/santi-api /opt/artifacts/santi-api

FROM ghcr.io/perishcode/docker/santi-runtime:v1 AS runtime

ENV HOME=/root \
    MODE=standalone \
    BIND_ADDR=0.0.0.0:8080 \
    SANTI_BASE_URL=http://127.0.0.1:8080 \
    STANDALONE_SQLITE_PATH=/data/santi-standalone.sqlite \
    EXECUTION_ROOT=/app \
    RUNTIME_ROOT=/runtime \
    RUST_LOG=santi_api=info

COPY --from=builder /opt/artifacts/santi-api /usr/local/bin/santi-api

EXPOSE 8080

CMD ["santi-api"]
