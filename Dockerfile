FROM rust:latest AS builder

# We need the nightly for some packages...
CMD rustup default nightly

WORKDIR /app

# Copy the source
COPY . .

# Useful for testing the docker file without installing all dependencies...
# RUN cargo init --bin --name sandpack-cdn

# Build (install) the binaries
RUN cargo build --release

# Runtime image
FROM rust:latest

# Run as "app" user
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /app/target/release/ /app/

RUN ls -la

# No CMD or ENTRYPOINT, see fly.toml with `cmd` override.
CMD /app/sandpack-cdn
