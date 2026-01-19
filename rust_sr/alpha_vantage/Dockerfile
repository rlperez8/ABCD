# -------------------------
# Builder stage
# -------------------------
FROM rustlang/rust:nightly as builder

WORKDIR /app
COPY . .

RUN cargo build --release

# -------------------------
# Runtime stage
# -------------------------
FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/alpha_vantage .

CMD ["./alpha_vantage"]
