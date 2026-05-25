# ---- Builder ----
FROM rust:alpine@sha256:606fd313a0f49743ee2a7bd49a0914bab7deedb12791f3a846a34a4711db7ed2 AS builder
WORKDIR /app

RUN apk add --no-cache musl-dev

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

# ---- Runtime ----
FROM alpine:3.22

WORKDIR /app
COPY --from=builder /app/target/release/poprako-b-preview /app/

CMD ["/app/poprako-b-preview"]
