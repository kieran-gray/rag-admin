FROM rust:1.96-trixie AS base

RUN rm -f /etc/apt/apt.conf.d/docker-clean \
    && echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' \
        > /etc/apt/apt.conf.d/keep-cache

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    apt-get update -y \
    && apt-get install -y --no-install-recommends clang

ADD https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz /tmp/cargo-binstall.tgz
RUN tar -xzf /tmp/cargo-binstall.tgz -C /usr/local/cargo/bin \
    && rm /tmp/cargo-binstall.tgz

RUN cargo binstall cargo-leptos -y

RUN rustup target add wasm32-unknown-unknown

WORKDIR /app

FROM base AS builder

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target,id=rag-admin-target \
    cargo leptos build --release -vv \
    && mkdir -p /out \
    && cp /app/target/release/rag-admin /out/ \
    && cp -r /app/target/site /out/site

FROM base AS dev

ENV LEPTOS_SITE_ADDR="0.0.0.0:3000"
ENV LEPTOS_RELOAD_PORT="3001"
ENV CARGO_TERM_COLOR="always"

EXPOSE 3000 3001

CMD ["cargo", "leptos", "watch"]

FROM debian:trixie-slim AS runtime
WORKDIR /app
RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates curl \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /out/rag-admin /app/
COPY --from=builder /out/site /app/site
COPY --from=builder /app/Cargo.toml /app/
COPY --from=builder /app/docs /app/docs

ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:3000"
ENV LEPTOS_SITE_ROOT="site"
EXPOSE 3000

CMD ["/app/rag-admin"]
