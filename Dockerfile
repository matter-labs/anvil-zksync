FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /anvil-zksync

FROM chef AS planner
COPY . .
# Mitigate rustup 1.28.0's new behavior https://blog.rust-lang.org/2025/03/02/Rustup-1.28.0.html
# Supposedly, it will be rolled back in the next patch release https://github.com/rust-lang/rustup/pull/4214
RUN rustup show active-toolchain || rustup toolchain install
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /anvil-zksync/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
# Mitigate rustup 1.28.0's new behavior https://blog.rust-lang.org/2025/03/02/Rustup-1.28.0.html
# Supposedly, it will be rolled back in the next patch release https://github.com/rust-lang/rustup/pull/4214
RUN rustup show active-toolchain || rustup toolchain install
RUN cargo build --release --bin anvil-zksync

FROM ubuntu:24.04 AS runtime

RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    && \
    rm -rf /var/lib/apt/lists/*

EXPOSE 8011

WORKDIR /usr/local/bin
COPY --from=builder /anvil-zksync/target/release/anvil-zksync .

ENTRYPOINT [ "anvil-zksync" ]
