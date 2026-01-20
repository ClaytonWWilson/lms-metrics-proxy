# Stage 1: Build the application
FROM rust:1.92-slim AS builder

WORKDIR /app

# Install build dependencies for SQLx
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (cached layer)
RUN cargo build --release && rm -rf src target/release/deps/token_counter*

# Copy actual source code
COPY src ./src

# Build the application
RUN cargo build --release

# Stage 2: Runtime image
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from builder
COPY --from=builder /app/target/release/token_counter /app/token_counter

# Expose the default port
EXPOSE 8080

# Run the application
CMD ["./token_counter"]
