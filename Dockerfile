FROM rust:bookworm AS builder

# We need the nightly for some packages...
CMD rustup default nightly
WORKDIR /app

RUN apt-get update && apt-get install -y protobuf-compiler libclang-dev libssl-dev

# Installing all dependencies...
RUN USER=root cargo new --bin sandpack-cdn
WORKDIR /app/sandpack-cdn
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release
RUN rm src/*.rs
RUN rm ./target/release/deps/sandpack_cdn*

# Copy the source
COPY . .

# Build (install) the binaries
RUN cargo build --release


FROM ubuntu:23.10

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata dumb-init \
    && rm -rf /var/lib/apt/lists/*

EXPOSE 8080
ENV APP_USER=appuser

RUN userdel ubuntu \
    && rm -rf /home/ubuntu \
    && groupadd $APP_USER \
    && useradd --create-home -g $APP_USER $APP_USER

WORKDIR /home/appuser

COPY --chown=$APP_USER:$APP_USER --from=builder /app/sandpack-cdn/target/release/  ./

USER $APP_USER
RUN mkdir /home/$APP_USER/npm_db

CMD ["dumb-init", "/home/appuser/sandpack-cdn"]
