FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /anvil-zksync

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /anvil-zksync/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin anvil-zksync

FROM ubuntu:22.04 as runtime

RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    && \
    rm -rf /var/lib/apt/lists/*

EXPOSE 8011

WORKDIR /usr/local/bin
COPY --from=builder /anvil-zksync/target/release/anvil-zksync .

ENTRYPOINT [ "anvil-zksync" ]
