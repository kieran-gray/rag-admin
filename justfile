set dotenv-load := true

default:
    @just --list

dev:
    cargo leptos watch

build:
    cargo leptos build --release

run: build
    cargo leptos serve --release

test:
    cargo test --features ssr

lint:
    cargo clippy --features ssr -- -D warnings
    cargo clippy --target wasm32-unknown-unknown --features hydrate -- -D warnings

lint-fix:
    cargo clippy --all-features --allow-dirty --fix

lint-check:
    cargo clippy --target wasm32-unknown-unknown -- -D warnings

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

up:
    docker compose up -d --build
    docker compose logs -f rag-admin

down:
    docker compose down

db-up:
    docker compose up -d db
