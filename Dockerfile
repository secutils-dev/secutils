# syntax=docker/dockerfile:1.2

FROM rust:1.98-slim-trixie@sha256:cc0448b41c3b7b7fea44f5dc50eacba729a56db365b65b7bd5e8a82d5b3db078 AS server_builder

ARG TARGETARCH
ARG UPX_VERSION=5.2.0

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
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:a77defd6fedbb3392b175ba8ea3d1c22be963c1597c248c3ba987ddd80bfb512
EXPOSE 7070

WORKDIR /app
COPY --from=server_builder ["/app/secutils", "./"]

CMD [ "./secutils" ]
