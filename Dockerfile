# Using official rust base image
FROM rust:1.68.2-alpine3.17
WORKDIR /moor
RUN apk add --no-cache musl-dev
RUN cargo install cargo-watch
EXPOSE 8080
COPY ./ ./

