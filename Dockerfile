FROM rust:latest as builder
WORKDIR /usr/src/ubee-exporter
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/ubee-exporter/target \
    cargo install --path .

FROM ubuntu:latest
RUN apt-get update && \
    apt-get install -y openssl && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/ubee-exporter /usr/local/bin/ubee-exporter
ENTRYPOINT ["ubee-exporter"]
