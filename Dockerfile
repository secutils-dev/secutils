# syntax=docker/dockerfile:1.2

FROM --platform=$BUILDPLATFORM rust:alpine3.18 as SERVER_BUILDER
ARG TARGETPLATFORM

## Statically link binary to OpenSSL libraries.
ENV OPENSSL_STATIC=yes
ENV OPENSSL_LIB_DIR=/usr/lib/
ENV OPENSSL_INCLUDE_DIR=/usr/include/

ENV PATH="$PATH:/app/aarch64-linux-musl-cross/bin"

WORKDIR /app
RUN set -x && apk add --no-cache pkgconfig musl-dev openssl-dev perl make curl

# Prepare environment: for cross compilation we download toolchain and `aarch64` OpenSSL libs.
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ]; \
    then set -x && \
        curl --remote-name-all https://musl.cc/aarch64-linux-musl-cross.tgz https://dl-cdn.alpinelinux.org/alpine/v3.18/main/aarch64/openssl-libs-static-3.1.1-r1.apk && \
        apk add --allow-untrusted openssl-libs-static-3.1.1-r1.apk && \
        tar xzf ./aarch64-linux-musl-cross.tgz && \
        rustup target add aarch64-unknown-linux-musl; \
    else set -x && \
        set -x && apk add --no-cache openssl-libs-static; \
    fi

# Fetch dependencies if they change.
COPY ["./Cargo.lock", "./Cargo.toml", "./"]
RUN set -x && cargo fetch

COPY [".", "./"]
RUN --mount=type=cache,target=/app/target if [ "$TARGETPLATFORM" = "linux/arm64" ]; \
    then set -x && \
        cargo test --target=aarch64-unknown-linux-musl --config target.aarch64-unknown-linux-musl.linker=\"aarch64-linux-musl-gcc\" && \
        cargo build --release --target=aarch64-unknown-linux-musl --config target.aarch64-unknown-linux-musl.linker=\"aarch64-linux-musl-gcc\" && \
        cp ./target/aarch64-unknown-linux-musl/release/secutils ./; \
    else set -x && \
        cargo test && \
        cargo build --release && \
        cp ./target/release/secutils ./; \
    fi

FROM alpine:3.18
WORKDIR /app
COPY --from=SERVER_BUILDER ["/app/secutils", "./"]
CMD [ "./secutils" ]
