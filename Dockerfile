# syntax=docker/dockerfile:1

FROM rust:1.100-slim-trixie@sha256:6f14b2b4b278ec0d50aa1d69541c304ce0f95e0f6268487aa9924af1c2e9aebe AS server_builder

ARG TARGETARCH
ARG UPX_VERSION=5.3.0

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
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2
EXPOSE 7070

WORKDIR /app
COPY --from=server_builder ["/app/secutils", "./"]

CMD [ "./secutils" ]
