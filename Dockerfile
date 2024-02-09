FROM rust:1.76.0-alpine AS builder

WORKDIR /app

RUN rustup target add x86_64-unknown-linux-musl

RUN apk update \
    && apk add protobuf-dev clang-dev openssl-dev lld musl-dev gcc openssl

ENV RUSTFLAGS="-C target-feature=+crt-static" \
    TARGET_CC=x86_64-linux-musl-gcc \
    OPENSSL_DIR=/etc/ssl

# RUN openssl version -d && exit 1

# Installing all dependencies...
RUN USER=root cargo new --bin sandpack-cdn
WORKDIR /app/sandpack-cdn
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm src/*.rs
RUN rm ./target/release/deps/sandpack_cdn*

# Copy the source
COPY . .

# Build (install) the binaries
RUN cargo build --release --target x86_64-unknown-linux-musl


FROM alpine

RUN apk update \
    && apk add ca-certificates tzdata dumb-init \
    && rm -rf /var/cache/apk/*

EXPOSE 8080
ENV APP_USER=appuser

RUN addgroup -S $APP_USER \
    && adduser -S $APP_USER -G $APP_USER

WORKDIR /home/appuser

COPY --chown=$APP_USER:$APP_USER --from=builder /app/sandpack-cdn/target/release/  ./

USER $APP_USER
RUN mkdir /home/$APP_USER/npm_db

CMD ["dumb-init", "/home/appuser/sandpack-cdn"]
