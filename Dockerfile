# syntax=docker/dockerfile:1.2

FROM --platform=$BUILDPLATFORM rust:alpine3.17 as SERVER_BUILDER
WORKDIR /app
RUN set -x && apk add --no-cache musl-dev openssl-dev make
COPY ["./Cargo.lock", "./Cargo.toml", "./.cargo", "./"]
RUN set -x && cargo fetch
COPY ["./src", "./src"]
RUN --mount=type=cache,target=/app/target \
    set -x && cargo test && cargo build --release && \
    cp ./target/release/secutils ./

FROM alpine:3.17
WORKDIR /app
COPY --from=SERVER_BUILDER ["/app/secutils", "./"]
CMD [ "./secutils" ]
