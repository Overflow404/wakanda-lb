FROM rust:1.88 as builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 && apt-get install -y netcat-openbsd
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/load-balancer .
RUN chmod +x /app/load-balancer
CMD ["/app/load-balancer"]
