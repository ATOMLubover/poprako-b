FROM rust:alpine
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

CMD ["/app/target/release/poprako-b-preview"]
