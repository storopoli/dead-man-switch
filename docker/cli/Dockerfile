# Build argument for package name
ARG CARGO_PACKAGE=dead-man-switch
ARG CARGO_PACKAGE_VARIANT=cli

############################################################
#                        builder
############################################################
FROM rust:alpine AS builder
# ^ see https://hub.docker.com/_/rust/tags?name=alpine

ARG CARGO_PACKAGE
ARG CARGO_PACKAGE_VARIANT

# Install build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static

# Set the working directory
WORKDIR /usr/src/app

# Copy workspace manifests
COPY ./Cargo.toml ./Cargo.lock ./

# Copy all crates
COPY ./crates ./crates

# Build application for release target
RUN cargo build -p ${CARGO_PACKAGE}-tui --release

############################################################
#                     final image
############################################################
FROM ghcr.io/linuxserver/baseimage-alpine:3.23 AS final
# ^ see https://github.com/linuxserver/docker-baseimage-alpine/pkgs/container/baseimage-alpine

ARG CARGO_PACKAGE
ARG CARGO_PACKAGE_VARIANT

# Set build arguments with defaults
ARG BUILD_DATE
ARG VERSION
ARG PUID=1000
ARG PGID=1000

# Environment variables
ENV PUID=${PUID} \
    PGID=${PGID}

# Install runtime dependencies
RUN apk add --no-cache ca-certificates tzdata && \
    rm -rf /var/cache/apk/*

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/${CARGO_PACKAGE}-tui /usr/local/bin/app

# Copy root filesystem (for s6-overlay services)
COPY docker/s6-overlay/_overrides/root/ /
COPY docker/s6-overlay/_common/root/ /
COPY docker/s6-overlay/${CARGO_PACKAGE_VARIANT}/root/ /

# Make binary executable
RUN chmod +x /usr/local/bin/app

# OCI annotations
LABEL org.opencontainers.image.title="DMS" \
      org.opencontainers.image.description="Dead Man Switch" \
      org.opencontainers.image.authors="storopoli" \
      org.opencontainers.image.source="https://github.com/storopoli/dead-man-switch" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.created="${BUILD_DATE}" \
      org.opencontainers.image.version="${VERSION}"

# Volumes for configuration
VOLUME /config
