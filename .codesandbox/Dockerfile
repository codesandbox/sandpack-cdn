FROM rust:1.69-bullseye

# install cargo binstall to reduce image size
WORKDIR /usr/local/cargo/bin
RUN curl -L --output cargo-binstall.tgz https://github.com/cargo-bins/cargo-binstall/releases/download/v0.22.0/cargo-binstall-x86_64-unknown-linux-gnu.tgz && \
    tar -xvzf cargo-binstall.tgz && \
    chmod +x cargo-binstall && \
    rm cargo-binstall.tgz

RUN apt-get update && apt-get install -y protobuf-compiler libclang-dev

RUN rustup component add rust-analyzer rustfmt rust-src clippy && \
    cargo binstall -y cargo-watch

WORKDIR /root
