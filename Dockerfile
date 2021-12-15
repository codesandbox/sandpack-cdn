FROM rust:latest AS builder

# We need the nightly for some packages...
CMD rustup default nightly

# Copy the source
COPY . .

# Build (install) the binaries
RUN cargo install --path .

# Runtime image
FROM debian:stretch

# Run as "app" user
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /root/.cargo/bin/ /app/

# No CMD or ENTRYPOINT, see fly.toml with `cmd` override.