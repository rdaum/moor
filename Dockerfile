# Multi-stage build: Frontend build stage
FROM node:20-bookworm AS frontend-build
WORKDIR /moor-frontend
# Install git for git hash lookup during build
RUN apt update && apt -y install git
COPY package.json package-lock.json* ./
RUN npm ci
COPY web-client/ ./web-client/
COPY tsconfig.json vite.config.ts ./
# Copy .git directory so vite can get the git hash during build
COPY ./.git ./.git
RUN npm run build

# Backend build stage
FROM rust:1.90-bookworm AS backend-build

WORKDIR /moor-build
RUN apt update
RUN apt -y install clang-16 libclang-16-dev swig python3-dev cmake libc6 git libsodium-dev pkg-config

# Stuff we'll need from the host to make the build work
COPY ./crates ./crates
COPY ./tools ./tools
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock

# We bring this over so we can get the git hash via shadow-rs. A bit bloated, but oh well.
COPY ./.git ./.git

# Build configuration: Use ARG to allow build-time customization
ARG BUILD_PROFILE=debug
ARG CARGO_BUILD_FLAGS=""
ARG TRACE_EVENTS=false
ARG CARGO_BUILD_JOBS=6

# Build flags here if you want optimal performance for your *particular* CPU,
# at the expense of portability.
# ENV RUSTFLAGS="-C target-cpu=native"

# Build either debug (fast) or release (optimized) based on BUILD_PROFILE
# Note: Cache mounts are ephemeral, so we copy binaries out to persist them in the image layer
# sharing=locked prevents race conditions when docker-compose builds multiple services in parallel
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/moor-build/target,sharing=locked \
    if [ "$BUILD_PROFILE" = "release" ]; then \
        if [ "$TRACE_EVENTS" = "true" ]; then \
            CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release --features trace_events -j $CARGO_BUILD_JOBS $CARGO_BUILD_FLAGS && \
            cp -r target/release /moor-build/target-final; \
        else \
            CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -j $CARGO_BUILD_JOBS $CARGO_BUILD_FLAGS && \
            cp -r target/release /moor-build/target-final; \
        fi \
    else \
        if [ "$TRACE_EVENTS" = "true" ]; then \
            cargo build --features trace_events -j $CARGO_BUILD_JOBS $CARGO_BUILD_FLAGS && \
            cp -r target/debug /moor-build/target-final; \
        else \
            cargo build -j $CARGO_BUILD_JOBS $CARGO_BUILD_FLAGS && \
            cp -r target/debug /moor-build/target-final; \
        fi \
    fi

# Runtime image - slim debian with just the essentials
FROM debian:bookworm-slim AS backend

# Re-declare ARG to use it in this stage
ARG BUILD_PROFILE=debug

# We need libssl for the curl worker, ca-certificates for HTTPS, and libsodium for CURVE encryption
RUN apt update && \
    apt -y install libssl3 ca-certificates libsodium23 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /moor

# The compiled service binaries from the backend build (debug or release depending on BUILD_PROFILE)
COPY --from=backend-build /moor-build/target-final/moor-daemon /moor/moor-daemon
COPY --from=backend-build /moor-build/target-final/moor-web-host /moor/moor-web-host
COPY --from=backend-build /moor-build/target-final/moor-telnet-host /moor/moor-telnet-host
COPY --from=backend-build /moor-build/target-final/moor-curl-worker /moor/moor-curl-worker

# The built web client static files from the frontend build
COPY --from=frontend-build /moor-frontend/dist /moor/web-client

# Utility binaries
COPY --from=backend-build /moor-build/target-final/moorc /moor/moorc
COPY --from=backend-build /moor-build/target-final/moor-emh /moor/moor-emh

EXPOSE 8080

# nginx-based frontend image
FROM nginx:alpine AS frontend
COPY --from=frontend-build /moor-frontend/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 80

# Default stage - backend services with moor binaries
FROM backend AS default
