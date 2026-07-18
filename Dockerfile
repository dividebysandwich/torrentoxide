############################  build stage  ############################
FROM rust:1-bookworm AS builder

# Build deps for the pure-Rust TLS stack (aws-lc-sys builds AWS-LC via cmake,
# whose assembly generation needs perl). A C toolchain ships with this image.
RUN apt-get update \
    && apt-get install -y --no-install-recommends cmake perl \
    && rm -rf /var/lib/apt/lists/*

# WASM target + the cargo-leptos build tool.
RUN rustup target add wasm32-unknown-unknown
RUN cargo install cargo-leptos --locked --version 0.3.2

WORKDIR /app
COPY . .

# Build the release server binary + hydrated wasm/js/css site (under target/site).
RUN cargo leptos build --release

###########################  runtime stage  ##########################
FROM debian:bookworm-slim AS runtime

# Only CA certificates are needed at runtime (TLS is pure-Rust; no OpenSSL).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/torrentoxide /app/torrentoxide
COPY --from=builder /app/target/site /app/site

# Where the built site lives + the address to bind (read by leptos at startup).
ENV LEPTOS_OUTPUT_NAME=torrentoxide \
    LEPTOS_SITE_ROOT=/app/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_ENV=PROD

# App defaults — downloads + resume state live under /data (mount a volume here).
ENV DOWNLOAD_DIR=/data/downloads \
    BROWSE_ROOT=/data/downloads \
    PERSISTENCE_DIR=/data/.rqbit-session \
    RUST_LOG=info,librqbit=warn

RUN mkdir -p /data/downloads /data/.rqbit-session

EXPOSE 3000
VOLUME ["/data"]

ENTRYPOINT ["/app/torrentoxide"]
