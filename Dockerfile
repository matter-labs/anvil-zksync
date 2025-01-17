FROM ubuntu:22.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
ENV DEBCONF_NONINTERACTIVE_SEEN=true

RUN apt-get update && apt-get install -y curl clang openssl libssl-dev gcc g++ \
    pkg-config build-essential libclang-dev linux-libc-dev liburing-dev && \
    rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y && \
    rustup install nightly-2024-08-01 && \
    rustup default nightly-2024-08-01

WORKDIR /usr/src/anvil-zksync
COPY . .

RUN cargo build --release

FROM ubuntu:22.04

RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    && \
    rm -rf /var/lib/apt/lists/*

EXPOSE 8011

WORKDIR /usr/local/bin
COPY --from=builder /usr/src/anvil-zksync/target/release/anvil-zksync .

ENTRYPOINT [ "anvil-zksync" ]
