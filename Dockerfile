FROM rust:1.72-alpine3.17 as builder
WORKDIR /usr/src/overmind
COPY . .
RUN apk add --no-cache musl-dev && cargo install --path .

FROM alpine:3.17
COPY --from=builder /usr/local/cargo/bin/overmind /usr/local/bin/overmind
CMD ["overmind"]