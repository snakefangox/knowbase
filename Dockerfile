FROM rust:1.72-alpine3.17 as builder
WORKDIR /usr/src/knowbase
COPY . .
RUN apk add --no-cache musl-dev && cargo install --path .

FROM alpine:3.17
LABEL org.opencontainers.image.source=https://github.com/daniel-swe/knowbase
COPY --from=builder /usr/local/cargo/bin/knowbase /usr/local/bin/knowbase
CMD ["knowbase"]