FROM rust:1.90-bookworm AS rust-builder

WORKDIR /usr/src/app
COPY . .

RUN cargo build --release -p bot_run

FROM debian:bookworm-slim AS debian-runtime

RUN mkdir -p /app
WORKDIR /app

# when build rust with native tls, it will link to libssl and libcrypto, so we need to install them in the runtime image
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=rust-builder /usr/src/app/target/release/bot_run /app/bot_run