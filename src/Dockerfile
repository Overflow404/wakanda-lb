FROM rust:1.88 as builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y netcat-openbsd
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/dummy-backend .
RUN chmod +x /app/dummy-backend
CMD ["/app/dummy-backend"]
