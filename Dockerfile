# Using official rust base image
FROM rust:1.81-bookworm
WORKDIR /moor
RUN apt update
RUN apt -y install clang libclang-dev swig python3-dev cmake libc6
EXPOSE 8080
COPY ./crates ./crates
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./JHCore-DEV-2.db ./JHCore-DEV-2.db
RUN CARGO_PROFILE_RELEASE_DEBUG=true cargo build --all-targets --release
