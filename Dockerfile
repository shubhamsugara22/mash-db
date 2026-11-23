# syntax=docker/dockerfile:1

FROM rust:1.90-slim-bookworm AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN useradd --system --uid 10001 --no-create-home mash \
    && mkdir -p /data \
    && chown mash:mash /data
COPY --from=builder /build/target/release/Mash_db /usr/local/bin/Mash_db
ENV MASH_DATA_DIR=/data \
    MASH_BIND=0.0.0.0:8080
VOLUME ["/data"]
EXPOSE 8080
USER mash
ENTRYPOINT ["/usr/local/bin/Mash_db"]
CMD ["serve"]
