# Using official rust base image
FROM rust:1.81-bookworm
WORKDIR /moor
RUN apt update
RUN apt -y install clang-16 libclang-16-dev swig python3-dev cmake libc6
# Generate the keypair for signing PASETO tokens. Shared between hosts and the daemon.
RUN openssl genpkey -algorithm ed25519 -out moor-signing-key.pem
RUN openssl pkey -in moor-signing-key.pem -pubout -out moor-verifying-key.pem
EXPOSE 8080
COPY ./crates ./crates
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./JHCore-DEV-2.db ./JHCore-DEV-2.db
RUN CARGO_PROFILE_RELEASE_DEBUG=true cargo build --all-targets --release
