# Debian 12 (bookworm) glibc build — works on most VDS
FROM rust:bookworm AS builder

WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN if [ ! -f Cargo.lock ]; then cargo generate-lockfile; fi

RUN cargo build --release \
    && strip target/release/nmos \
    && ./target/release/nmos --help >/dev/null

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl wget \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/nmos /usr/local/bin/nmos
ENTRYPOINT ["nmos"]

FROM debian:bookworm-slim AS export
COPY --from=builder /app/target/release/nmos /nmos
