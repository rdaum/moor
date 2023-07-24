# Using official rust base image
FROM rust:1.71-bullseye
WORKDIR /moor
RUN apt install libclang-dev
RUN cargo install cargo-watch
EXPOSE 8080
COPY ./ ./

