# Using official rust base image
FROM rust:1.78-bullseye
WORKDIR /moor
RUN apt update
RUN apt -y install clang libclang-dev swig python3-dev cmake
RUN cargo install cargo-watch
EXPOSE 8080
COPY ./ ./

