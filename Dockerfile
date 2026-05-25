# ---- Builder ----
FROM rust:alpine AS builder
WORKDIR /app

RUN apk add --no-cache musl-dev

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

# ---- Runtime ----
FROM alpine:3.22

WORKDIR /app
COPY --from=builder /app/target/release/poprako-b-preview /app/

# Mount point for the memory volume (see docker-compose.yml).
# The directory is empty in the image; runtime content comes
# from the host via volume mount.
RUN mkdir -p /app/memory

CMD ["/app/poprako-b-preview"]
