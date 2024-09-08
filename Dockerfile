# Start from the official Rust image
FROM rust:latest AS builder

# Set the working directory
WORKDIR /usr/src/app

# Add labels for OCI annotations
LABEL org.opencontainers.image.source="https://github.com/storopoli/dead-man-switch" \
    org.opencontainers.image.description="Dead Man's Switch" \
    org.opencontainers.image.licenses="AGPLv3"

# Copy project's source
COPY ./Cargo.toml ./
COPY ./Cargo.lock ./
COPY ./crates ./crates

# Build application for release target
RUN cargo build -p dead-man-switch-web --release

# Start a new stage from a slim version of Debian to reduce the size of the final image
FROM debian:bookworm-slim

# Install libssl3
RUN apt-get update && apt-get install -y libssl3 && apt clean && rm -rf /var/lib/apt/lists/*


WORKDIR /usr/src/app

# Copy the binary from the builder stage to the new stage
COPY --from=builder /usr/src/app/target/release/dead-man-switch-web /usr/local/bin/dead-man-switch-web

# Expose port 3000
EXPOSE 3000

# Command to run the binary
CMD ["dead-man-switch-web"]
