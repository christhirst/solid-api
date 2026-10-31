# syntax=docker/dockerfile:1
# ==============================================================================
# Stage 1: Builder
# ==============================================================================
FROM rust:1-bookworm AS builder

WORKDIR /app

# Install protobuf compiler required by tonic-prost-build
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
    libprotobuf-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests and proto definitions
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto
COPY src ./src
COPY config ./config

# Build release binary
RUN cargo build --release --locked && strip /app/target/release/solid-api

# ==============================================================================
# Stage 2: Distroless Runtime
# ==============================================================================
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

WORKDIR /app

# Copy compiled binary and default configuration
COPY --from=builder /app/target/release/solid-api /app/solid-api
COPY --from=builder /app/config /app/config

ENV RUST_LOG=info,solid_api=info
ENV SOLID_API_CONFIG_PATH=/app/config/default.toml

EXPOSE 50051

USER nonroot:nonroot

ENTRYPOINT ["/app/solid-api"]
CMD ["serve"]
