FROM lukemathwalker/cargo-chef:latest-rust-alpine AS base
RUN apk add sccache
ENV RUSTC_WRAPPER=sccache SCCACHE_DIR=/sccache
WORKDIR /app

# Using `cargo chef` to cache dependencies and improve build times
# See https://github.com/LukeMathWalker/cargo-chef
FROM base AS planner
COPY firefly-cardanoconnect /app/firefly-cardanoconnect
COPY firefly-cardanosigner /app/firefly-cardanosigner
COPY firefly-server /app/firefly-server
COPY Cargo.toml Cargo.lock /app/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo chef prepare --recipe-path recipe.json

# Building all binaries in the workspace at once, to avoid duplicate work
FROM base AS builder 
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo chef cook --release --workspace --recipe-path recipe.json
COPY firefly-cardanoconnect /app/firefly-cardanoconnect
COPY firefly-cardanosigner /app/firefly-cardanosigner
COPY firefly-server /app/firefly-server
COPY Cargo.toml Cargo.lock /app/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo build --release
