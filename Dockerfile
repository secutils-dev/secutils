# syntax=docker/dockerfile:1.2

FROM --platform=$BUILDPLATFORM rust:1.88-slim-bookworm AS server_builder
ARG TARGETPLATFORM

## Statically link binary to OpenSSL libraries.
ENV OPENSSL_STATIC=yes \
    AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_LIB_DIR=/usr/lib/aarch64-linux-gnu \
    AARCH64_UNKNOWN_LINUX_GNU_OPENSSL_INCLUDE_DIR=/usr/include/aarch64-linux-gnu \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++ \
    PKG_CONFIG_PATH="/usr/lib/aarch64-linux-gnu/pkgconfig/:${PKG_CONFIG_PATH}"

WORKDIR /app

# Install dependencies (including cross-compilation toolchain).
RUN set -x && \
    dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install -y pkg-config curl libssl-dev cmake libssl-dev:arm64 g++-aarch64-linux-gnu libc6-dev-arm64-cross protobuf-compiler ca-certificates && \
    rustup target add aarch64-unknown-linux-gnu

# Copy assets, member crates, and manifest.
COPY ["./assets", "./assets"]
COPY ["./components/secutils-jwt-tools", "./components/secutils-jwt-tools"]
COPY ["./Cargo.lock", "./Cargo.toml", "./"]

# Fetch dependencies if they change.
RUN set -x && cargo fetch

# Copy source code and build.
COPY [".", "./"]
RUN --mount=type=cache,target=/app/target if [ "$TARGETPLATFORM" = "linux/arm64" ]; \
    then set -x && \
        cargo build --release --target=aarch64-unknown-linux-gnu && \
        cp ./target/aarch64-unknown-linux-gnu/release/secutils ./; \
    else set -x && \
        cargo build --release && \
        cp ./target/release/secutils ./; \
    fi

FROM debian:bookworm-slim
EXPOSE 7070

ENV APP_USER=secutils
ENV APP_USER_UID=1001

WORKDIR /app
COPY --from=server_builder ["/app/secutils", "./"]
COPY --from=server_builder ["/etc/ssl/certs/ca-certificates.crt", "/etc/ssl/certs/"]

# Configure group and user.
RUN addgroup --system --gid $APP_USER_UID $APP_USER \
    && adduser --system --uid $APP_USER_UID --ingroup $APP_USER $APP_USER
RUN chown -R $APP_USER:$APP_USER ./
USER $APP_USER

CMD [ "./secutils" ]
