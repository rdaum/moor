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
FROM rust:1.88-bookworm AS backend-build
WORKDIR /moor-build
RUN apt update
RUN apt -y install clang-16 libclang-16-dev swig python3-dev cmake libc6 git

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

# Build either debug (fast) or release (optimized) based on BUILD_PROFILE
RUN if [ "$BUILD_PROFILE" = "release" ]; then \
        CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release $CARGO_BUILD_FLAGS; \
    else \
        cargo build $CARGO_BUILD_FLAGS; \
    fi

# But we don't need the source code and all the rust stuff and packages in our final image. Just slim.
FROM linuxcontainers/debian-slim:latest AS backend

# Pass the build profile to the final stage
ARG BUILD_PROFILE=debug

# We need libssl for the curl worker and ca-certificates for HTTPS
RUN apt update
RUN apt -y install libssl3 ca-certificates

WORKDIR /moor

# The keys for signing and verifying PASETO tokens, we built them in the backend build image
COPY --from=backend-build ./moor-build/moor-signing-key.pem ./moor-signing-key.pem
COPY --from=backend-build ./moor-build/moor-verifying-key.pem ./moor-verifying-key.pem

# The compiled service binaries from the backend build (debug or release depending on BUILD_PROFILE)
COPY --from=backend-build /moor-build/target/${BUILD_PROFILE}/moor-daemon /moor/moor-daemon
COPY --from=backend-build /moor-build/target/${BUILD_PROFILE}/moor-web-host /moor/moor-web-host
COPY --from=backend-build /moor-build/target/${BUILD_PROFILE}/moor-telnet-host /moor/moor-telnet-host
COPY --from=backend-build /moor-build/target/${BUILD_PROFILE}/moor-curl-worker /moor/moor-curl-worker

# The built web client static files from the frontend build
COPY --from=frontend-build /moor-frontend/dist /moor/web-client

# `moorc` binary can be used to compile objdef or textdump sources without running a full daemon
COPY --from=backend-build /moor-build/target/${BUILD_PROFILE}/moorc /moor/moorc

EXPOSE 8080

# nginx-based frontend image
FROM nginx:alpine AS frontend
COPY --from=frontend-build /moor-frontend/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf
EXPOSE 80

# Default stage - backend services with moor binaries
FROM backend AS default
