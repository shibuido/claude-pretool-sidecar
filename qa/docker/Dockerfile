# QA environment for claude-pretool-sidecar
#
# Multi-stage build:
#   Stage 1: Build the Rust binaries
#   Stage 2: Minimal runtime with QA scripts and test dependencies
#
# Docker build optimization: dependency layer cached separately from source.

# --- Stage 1: Build ---
FROM rust:1.86-slim AS builder

WORKDIR /build

# Layer 1: Install system dependencies (most stable)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Layer 2: Cache Rust dependencies (copy manifests first)
COPY Cargo.toml Cargo.lock ./
# Create dummy source to build dependencies
RUN mkdir -p src/bin && \
    echo 'fn main() { println!("dummy"); }' > src/main.rs && \
    echo 'fn main() { println!("dummy"); }' > src/bin/logger.rs && \
    cargo build --release 2>/dev/null || true && \
    cargo build --release --tests 2>/dev/null || true

# Layer 3: Copy actual source and rebuild (only this layer invalidates on code changes)
COPY src/ src/
COPY tests/ tests/
RUN touch src/main.rs src/bin/logger.rs && \
    cargo build --release && \
    cargo test --release --no-run

# --- Stage 2: QA Runtime ---
FROM debian:bookworm-slim AS qa

LABEL maintainer="shibuido" \
      description="QA environment for claude-pretool-sidecar" \
      org.opencontainers.image.source="https://github.com/shibuido/claude-pretool-sidecar"

# Install runtime dependencies for QA scripts
RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    jq \
    coreutils \
    findutils \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /qa

# Copy built binaries
COPY --from=builder /build/target/release/claude-pretool-sidecar /usr/local/bin/
COPY --from=builder /build/target/release/claude-pretool-logger /usr/local/bin/

# Copy test binaries for cargo test
COPY --from=builder /build/target/release/deps/claude_pretool_sidecar-* /qa/cargo-tests/
COPY --from=builder /build/target/release/deps/integration_test-* /qa/cargo-tests/

# Copy QA suite
COPY qa/ /qa/
COPY tests/fixtures/ /qa/rust-test-fixtures/
COPY examples/ /qa/examples/

# Ensure scripts are executable
RUN chmod +x /qa/scripts/*.sh /qa/helpers/*.sh /qa/docker/*.sh 2>/dev/null || true

# Default: run all QA tests
ENTRYPOINT ["/qa/scripts/run-all-qa.sh"]
CMD ["--skip-cargo"]
