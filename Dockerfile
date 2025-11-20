FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml ./

COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim

WORKDIR /app

# Install OpenSSL and CA certificates
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/uj-ai-club-backend /app/uj-ai-club-backend

# Create uploads directory
RUN mkdir -p /app/uploads/avatars

EXPOSE 8000

ENV RUST_LOG=info
ENV SERVER_ADDRESS=0.0.0.0:8000
ENV JWT_SECRET=your_jwt_secret

CMD ["/app/uj-ai-club-backend"]
