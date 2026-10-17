# enclave-daemon/Dockerfile
FROM rust:1.82-slim AS builder
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo test --release && cargo build --release --bin obsidian-daemon

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/obsidian-daemon /usr/local/bin/obsidian-daemon
ENV PORT=8080
EXPOSE 8080
CMD ["obsidian-daemon"]