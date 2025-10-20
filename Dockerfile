# Build argument to control target architecture (amd64 or arm64)
# TARGETARCH is automatically set by Docker BuildKit to the builder's native architecture
# TARGET_ARCH defaults to native, but can be overridden for cross-compilation: --build-arg TARGET_ARCH=arm64
ARG TARGETARCH
ARG TARGET_ARCH=${TARGETARCH:-amd64}

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
# Uses the builder's native platform for fast compilation
# Cross-compilation to different architectures happens via cargo's --target flag
FROM rust:1.88-bookworm AS backend-build

# Re-declare ARG to use it in this stage (global ARGs need re-declaration)
ARG TARGET_ARCH

WORKDIR /moor-build
RUN apt update
RUN apt -y install clang-16 libclang-16-dev swig python3-dev cmake libc6 git

# Install ARM64 cross-compilation toolchain if building for ARM64
RUN if [ "$TARGET_ARCH" = "arm64" ]; then \
        dpkg --add-architecture arm64 && \
        apt update && \
        apt -y install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu \
            libssl-dev:arm64 pkg-config && \
        rustup target add aarch64-unknown-linux-gnu; \
    fi

# Configure pkg-config for ARM64 cross-compilation
ENV PKG_CONFIG_ALLOW_CROSS=1

# Generate the keypair for signing PASETO tokens. Shared between hosts and the daemon.
RUN openssl genpkey -algorithm ed25519 -out moor-signing-key.pem
RUN openssl pkey -in moor-signing-key.pem -pubout -out moor-verifying-key.pem

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

# Build flags here if you want optimal performance for your *particular* CPU,
# at the expense of portability.
# ENV RUSTFLAGS="-C target-cpu=native"

# Determine the Rust target triple based on target architecture
RUN if [ "$TARGET_ARCH" = "arm64" ]; then \
        echo "aarch64-unknown-linux-gnu" > /tmp/rust-target; \
    else \
        echo "x86_64-unknown-linux-gnu" > /tmp/rust-target; \
    fi

# Build either debug (fast) or release (optimized) based on BUILD_PROFILE
# Cross-compile to ARM64 if TARGET_ARCH is arm64
# Note: Cache mounts are ephemeral, so we copy binaries out to persist them in the image layer
# sharing=locked prevents race conditions when docker-compose builds multiple services in parallel
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/moor-build/target,sharing=locked \
    RUST_TARGET=$(cat /tmp/rust-target) && \
    if [ "$TARGET_ARCH" = "arm64" ]; then \
        export PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig && \
        export PKG_CONFIG_SYSROOT_DIR=/; \
    fi && \
    if [ "$BUILD_PROFILE" = "release" ]; then \
        CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release --target $RUST_TARGET -j 6 $CARGO_BUILD_FLAGS && \
        cp -r target/$RUST_TARGET/release /moor-build/target-final; \
    else \
        cargo build --target $RUST_TARGET -j 6 $CARGO_BUILD_FLAGS && \
        cp -r target/$RUST_TARGET/debug /moor-build/target-final; \
    fi

# But we don't need the source code and all the rust stuff and packages in our final image. Just slim.
# The runtime image architecture should match what we cross-compiled for
FROM --platform=linux/${TARGET_ARCH} linuxcontainers/debian-slim:latest AS backend

# Re-declare ARGs to use them in this stage
ARG TARGET_ARCH
ARG BUILD_PROFILE=debug

# We need libssl for the curl worker and ca-certificates for HTTPS
RUN apt update
RUN apt -y install libssl3 ca-certificates

WORKDIR /moor

# The keys for signing and verifying PASETO tokens, we built them in the backend build image
COPY --from=backend-build ./moor-build/moor-signing-key.pem ./moor-signing-key.pem
COPY --from=backend-build ./moor-build/moor-verifying-key.pem ./moor-verifying-key.pem

# The compiled service binaries from the backend build (debug or release depending on BUILD_PROFILE)
COPY --from=backend-build /moor-build/target-final/moor-daemon /moor/moor-daemon
COPY --from=backend-build /moor-build/target-final/moor-web-host /moor/moor-web-host
COPY --from=backend-build /moor-build/target-final/moor-telnet-host /moor/moor-telnet-host
COPY --from=backend-build /moor-build/target-final/moor-curl-worker /moor/moor-curl-worker

# The built web client static files from the frontend build
COPY --from=frontend-build /moor-frontend/dist /moor/web-client

# Utility binaries
COPY --from=backend-build /moor-build/target-final/moorc /moor/moorc
COPY --from=backend-build /moor-build/target-final/moor-admin /moor/moor-admin

EXPOSE 8080

# nginx-based frontend image
FROM --platform=linux/${TARGET_ARCH} nginx:alpine AS frontend
# Re-declare ARG to use it in this stage
ARG TARGET_ARCH
COPY --from=frontend-build /moor-frontend/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 80

# Default stage - backend services with moor binaries
FROM backend AS default
