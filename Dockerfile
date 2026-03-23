FROM rust:1.88-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ARG SANTI_UID=1000
ARG SANTI_GID=1000

RUN groupadd --gid ${SANTI_GID} santi \
    && useradd --uid ${SANTI_UID} --gid ${SANTI_GID} --create-home --shell /bin/bash santi \
    && mkdir -p /var/lib/santi/runtime /var/lib/santi /home/santi/target \
    && chown -R santi:santi /var/lib/santi /home/santi /usr/local/cargo /usr/local/rustup

WORKDIR /app/api

ENV HOME=/home/santi \
    CARGO_TARGET_DIR=/home/santi/target \
    BIND_ADDR=0.0.0.0:8080 \
    DATABASE_URL=postgres://santi:santi@postgres:5432/santi \
    EXECUTION_ROOT=/app \
    RUNTIME_ROOT=/var/lib/santi/runtime \
    RUST_LOG=santi_api=info

COPY api/Cargo.toml /app/api/Cargo.toml
COPY api/Cargo.lock /app/api/Cargo.lock
COPY api/src /app/api/src
COPY api/tests /app/api/tests

USER santi

EXPOSE 8080

CMD ["cargo", "run"]
