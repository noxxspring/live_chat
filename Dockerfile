# =============================================================================
# Stage 1: Builder - Build the binary
# =============================================================================
FROM rust:1.85-slim as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifest files first (for dependency caching)
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies (caches them)
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src target/release/deps/live_chat* target/release/live_chat*

# Copy source code and static files
COPY src ./src
COPY static ./static

# Touch main.rs to force Cargo to recompile the binary with actual source code
RUN touch src/main.rs && cargo build --release

# =============================================================================
# Stage 2: Runtime - Minimal image
# =============================================================================
FROM debian:bookworm-slim

# Install runtime dependencies (ca-certificates, openssl, and libssl3)
RUN apt-get update && \
    apt-get install -y ca-certificates openssl libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN adduser --system --no-create-home --group appuser

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/live_chat /usr/local/bin/live_chat

# Copy static files
COPY --from=builder /app/static /opt/live_chat/static

# Set working directory
WORKDIR /opt/live_chat

# Run as non-root user
USER appuser

# Expose the application port
EXPOSE 3030

# Start the application
CMD ["/usr/local/bin/live_chat"]