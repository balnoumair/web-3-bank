# Build context must be repo root (needs packages/proto/ for build.rs)
ARG RUST_VERSION=1.77.0
FROM rust:${RUST_VERSION} AS builder
WORKDIR /app

# Install protoc (required by tonic-build)
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Proto files needed by build.rs
COPY packages/proto/ packages/proto/

# Copy lock file for reproducible builds, then service source
COPY services/Cargo.lock services/Cargo.lock
COPY services/treasury/ services/treasury/

# Override to a single-member workspace — avoids copying user-service/ into context
RUN printf '[workspace]\nmembers = ["treasury"]\nresolver = "2"\n' > services/Cargo.toml

# Build from workspace root; binary lands at services/target/release/treasury
WORKDIR /app/services
RUN SQLX_OFFLINE=true cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/services/target/release/treasury /usr/local/bin/
EXPOSE 50052
CMD ["treasury"]
