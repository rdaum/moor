# Using official rust base image
FROM rust:1.71-bullseye
WORKDIR /moor
RUN apt update
RUN apt -y install clang libclang-dev
RUN cargo install cargo-watch
EXPOSE 8080
COPY ./ ./

