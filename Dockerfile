# syntax=docker/dockerfile:1.2

FROM rust:1.90-slim-trixie AS server_builder

ARG TARGETARCH
ARG UPX_VERSION=5.0.2

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

# Copy assets, member crates, and manifest.
COPY ["./assets", "./assets"]
COPY ["./components/secutils-jwt-tools", "./components/secutils-jwt-tools"]
COPY ["./Cargo.lock", "./Cargo.toml", "./"]

# Fetch dependencies if they change.
RUN set -x && cargo fetch

# Copy source code and build.
COPY [".", "./"]
RUN --mount=type=cache,target=/app/target set -x && cargo build --release && \
    cp ./target/release/secutils ./ && \
    upx --best --lzma ./secutils

# Check out https://gcr.io/distroless/cc-debian12:nonroot
FROM gcr.io/distroless/cc-debian12:nonroot
EXPOSE 7070

WORKDIR /app
COPY --from=server_builder ["/app/secutils", "./"]

CMD [ "./secutils" ]
