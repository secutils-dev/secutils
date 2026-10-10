# syntax=docker/dockerfile:1

FROM rust:1.99-slim-trixie@sha256:2752b332db73fdbb7dc576f06c82ed1f312005784ef913d7e04a28f5f55dc581 AS server_builder

ARG TARGETARCH
ARG UPX_VERSION=5.2.1

## Statically link binary to OpenSSL libraries.
ENV OPENSSL_STATIC=yes

WORKDIR /app

# Install dependencies.
RUN set -x && \
    apt-get update && \
    apt-get install -y pkg-config curl libssl-dev cmake g++ protobuf-compiler curl xz-utils ca-certificates

# Download and install UPX.
RUN curl -LO https://github.com/upx/upx/releases/download/v${UPX_VERSION}/upx-${UPX_VERSION}-${TARGETARCH}_linux.tar.xz && \
    tar -xf upx-${UPX_VERSION}-${TARGETARCH}_linux.tar.xz && \
    mv upx-${UPX_VERSION}-${TARGETARCH}_linux/upx /usr/local/bin/ && \
    rm -rf upx-${UPX_VERSION}-${TARGETARCH}_linux.tar.xz upx-${UPX_VERSION}-${TARGETARCH}_linux

# Copy assets, member crates, submodule crates, and manifest.
COPY ["./assets", "./assets"]
COPY ["./components/secutils-jwt-tools", "./components/secutils-jwt-tools"]
COPY ["./components/retrack/components/retrack-types", "./components/retrack/components/retrack-types"]
# `benches/js-runtime-perf` is a workspace member; `cargo fetch`/`cargo build`
# need its manifest (and sources) to resolve the workspace graph, even though
# the binary build below does not compile it.
COPY ["./benches", "./benches"]
COPY ["./Cargo.lock", "./Cargo.toml", "./"]

# Fetch dependencies if they change.
RUN set -x && cargo fetch

# Copy only the files needed for the Rust build.

COPY ["./.cargo", "./.cargo"]
COPY ["./build.rs", "./"]
COPY ["./.sqlx", "./.sqlx"]
COPY ["./migrations", "./migrations"]
COPY ["./src", "./src"]
RUN --mount=type=cache,target=/app/target set -x && cargo build --release && \
    cp ./target/release/secutils ./ && \
    upx --best --lzma ./secutils

# Check out https://gcr.io/distroless/cc-debian13:nonroot
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2
EXPOSE 7070

WORKDIR /app
COPY --from=server_builder ["/app/secutils", "./"]

CMD [ "./secutils" ]
