ARG TARGET_DEFAULT=x86_64-unknown-linux-musl

#################################
# Build prellblock
#################################
#FROM rustlang/rust:nightly-alpine3.12 as builder
FROM rustlang/rust:nightly-buster-slim as builder

ARG TARGET_DEFAULT
ENV TARGET=$TARGET_DEFAULT

# RUN apk update && \
#     apk add --no-cache \
#     build-base \
#     openssl-dev \
#     && true
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential \
    libssl-dev \
    musl-tools \ 
    && true

COPY . /src
WORKDIR /src

RUN rustup target add ${TARGET}

RUN RUST_BACKTRACE=full cargo build --release --target=${TARGET}

#################################
# Copy compiled version
#################################
FROM alpine:3.12

ARG TARGET_DEFAULT
WORKDIR /prellblock
COPY --from=builder /src/target/$TARGET_DEFAULT/release/prellblock .
ENTRYPOINT ["/prellblock/prellblock"]
