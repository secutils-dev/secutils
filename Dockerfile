# syntax=docker/dockerfile:1.2

FROM --platform=$BUILDPLATFORM rust:alpine3.17 as SERVER_BUILDER
ARG TARGETPLATFORM
WORKDIR /app
RUN set -x && apk add --no-cache musl-dev openssl-dev perl make curl
# Prepare environment for the cross-compilation to arm64.
RUN if [ "$TARGETPLATFORM" = "linux/arm64" ]; \
    then set -x && \
         curl -O https://musl.cc/aarch64-linux-musl-cross.tgz && \
         tar xzf ./aarch64-linux-musl-cross.tgz && \
         rustup target add aarch64-unknown-linux-musl; \
    fi
COPY ["./Cargo.lock", "./Cargo.toml", "./sqlx-data.json", "./"]
RUN set -x && cargo fetch
COPY ["./src", "./src"]
COPY ["./migrations", "./migrations"]
RUN --mount=type=cache,target=/app/target if [ "$TARGETPLATFORM" = "linux/arm64" ]; \
    then set -x && \
         export PATH="/app/aarch64-linux-musl-cross/bin:${PATH}" && \
         cargo test && \
         cargo build --release --target=aarch64-unknown-linux-musl --config target.aarch64-unknown-linux-musl.linker=\"aarch64-linux-musl-gcc\" && \
         cp ./target/aarch64-unknown-linux-musl/release/secutils ./; \
    else set -x && \
         cargo test && \
         cargo build --release && \
         cp ./target/release/secutils ./; \
    fi

FROM alpine:3.17
WORKDIR /app
COPY --from=SERVER_BUILDER ["/app/secutils", "./"]
CMD [ "./secutils" ]
