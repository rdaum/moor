# Using official rust base image for building the project.
FROM rust:1.84-bookworm AS build
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

RUN CARGO_PROFILE_RELEASE_DEBUG=true cargo build --all-targets --release
COPY ./crates/web-host/src/client ./client

# But we don't need the source code and all the rust stuff and packages in our final image. Just slim.
FROM linuxcontainers/debian-slim:latest

WORKDIR /moor

# The keys for signing and verifying PASETO tokens, we built them in the build image. We could do them here, but then
# we'd have to drag openssl in, so why bother.
COPY --from=build ./moor-build/moor-signing-key.pem ./moor-signing-key.pem
COPY --from=build ./moor-build/moor-verifying-key.pem ./moor-verifying-key.pem

# The compiled service binaries from the build
COPY --from=build /moor-build/target/release/moor-daemon /moor/moor-daemon
COPY --from=build /moor-build/target/release/moor-web-host /moor/moor-web-host
COPY --from=build /moor-build/target/release/moor-telnet-host /moor/moor-telnet-host

# The web client source directory
COPY --from=build /moor-build/client /moor/client

# `moorc` binary can be used to compile objdef or textdump sources without running a full daemon
COPY --from=build /moor-build/target/release/moorc /moor/moorc

EXPOSE 8080
