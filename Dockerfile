# Backend build stage
FROM rust:1.92-bookworm AS backend-build

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

# Build either debug (fast) or release (optimized) based on BUILD_PROFILE
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/moor-build/target,sharing=locked \
    if [ "$BUILD_PROFILE" = "release" ] || [ "$BUILD_PROFILE" = "release-fast" ]; then \
        PROFILE_FLAG="--profile $BUILD_PROFILE"; \
        if [ "$BUILD_PROFILE" = "release" ]; then PROFILE_FLAG="--release"; fi; \
        if [ "$TRACE_EVENTS" = "true" ]; then \
            cargo build $PROFILE_FLAG --features trace_events -j $CARGO_BUILD_JOBS $CARGO_BUILD_FLAGS && \
            cp -r target/$BUILD_PROFILE /moor-build/target-final; \
        else \
            cargo build $PROFILE_FLAG -j $CARGO_BUILD_JOBS $CARGO_BUILD_FLAGS && \
            cp -r target/$BUILD_PROFILE /moor-build/target-final; \
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

# Utility binaries
COPY --from=backend-build /moor-build/target-final/moorc /moor/moorc
COPY --from=backend-build /moor-build/target-final/moor-emh /moor/moor-emh
COPY --from=backend-build /moor-build/target-final/moor-mcp-host /moor/moor-mcp-host

EXPOSE 8080

# Default stage - backend services with moor binaries
FROM backend AS default
