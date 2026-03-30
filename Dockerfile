FROM rust:1.86-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/kalshi-bot .
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/config ./config

ENV PORT=8080
EXPOSE 8080

CMD ["./kalshi-bot"]
