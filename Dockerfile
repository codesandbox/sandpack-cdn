FROM rust:bookworm AS builder

# We need the nightly for some packages...
CMD rustup default nightly
RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /app

RUN apt-get update && apt-get install -y protobuf-compiler libclang-dev libssl-dev

# Installing all dependencies...
RUN USER=root cargo new --bin sandpack-cdn
WORKDIR /app/sandpack-cdn
COPY Cargo.toml Cargo.lock ./
RUN cargo build --target x86_64-unknown-linux-musl --release
RUN rm src/*.rs
RUN rm ./target/release/deps/sandpack_cdn*

# Copy the source
COPY . .

# Build (install) the binaries
RUN cargo build --target x86_64-unknown-linux-musl --release


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
