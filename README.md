# rag-admin

A Work-In-Progress local-only Leptos app for evaluating retrieval pipelines against your own
documents. Pull source documents in, run different chunking strategies and
embedding models against them, generate synthetic Q&A datasets from the content,
then score retrieval quality across pipeline variants.

Not deployed. Run it locally, point it at a content source and a vector store,
and iterate on what gives you the best recall/precision before promoting a
pipeline to production.

## Run

The recommended way to run the app is via Docker Compose, which brings up
Postgres (with pgvector), Ollama, and the app itself in one go.

Prerequisites:

- [Docker](https://docs.docker.com/get-docker/) (with Compose)
- Optional: [just](https://github.com/casey/just) for convenience recipes
- Optional: [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html)
  if you have an NVIDIA GPU and want Ollama to use it. See
  [mythrantic/ollama-docker](https://github.com/mythrantic/ollama-docker) for a
  worked example.

Copy `.env.example` to `.env`, then start the stack:

```sh
docker compose up -d
# or, with just:
just up
```

Then open `http://localhost:3000`.

### First run: model downloads

The Compose stack starts Ollama, stores its model files in the `ollama-data`
volume, and populates the local models seeded by the app:

- `qwen3-embedding:0.6b` for embeddings
- `gemma3:12b-it-qat` for generation

The first run downloads these models and can take a while. **The defaults are
not suitable for all hardware** — `gemma3:12b-it-qat` needs roughly 12–16 GB of
RAM/VRAM to run comfortably. If your machine can't fit it, edit
`docker-compose.yml` (the `ollama-pull-*` services) to pull smaller variants
such as `gemma3:4b-it-qat` or `gemma3:1b-it-qat` and adjust the models in the
UI accordingly.

Adjust `OLLAMA_MEMORY_RESERVATION` and `OLLAMA_MEMORY_LIMIT` in `.env` if Docker
does not have enough memory available for the generation model. Keep at least
20 GB free in Docker's volume store for the Ollama model blobs, partial
downloads, and future model updates.

### Local development (without the app container)

If you want to run the Rust app on the host with hot reload and only Postgres
in Docker:

```sh
just db-up         # docker compose up -d db
just dev           # cargo leptos watch
```

This needs the Rust toolchain plus `cargo-leptos` installed locally.

Run `just` with no arguments to see all available recipes (build, test, lint,
format, …).

## Configuration

Environment variables (see `.env.example`):

| Variable | Purpose |
| --- | --- |
| `DATABASE_URL` | Postgres connection string (pgvector required) |
| `OLLAMA_BASE_URL` | Optional. Defaults to `http://localhost:11434` |
| `OLLAMA_MEMORY_RESERVATION`, `OLLAMA_MEMORY_LIMIT` | Docker Compose memory settings for the local Ollama service |
| `OLLAMA_CONTEXT_LENGTH` | Ollama generation context size. Defaults to `16384` |
| `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN` | Required if using Cloudflare Workers AI / Vectorize |
| `CLOUDFLARE_KV_NAMESPACE_ID` | Required when `KV_BACKEND=cloudflare` |
| `KV_BACKEND` | `postgres` (default) or `cloudflare` |

Embedding models, generation models, vector indexes, and pipeline configurations
are managed at runtime through the UI and stored in Postgres.

## Stack

Leptos (SSR + hydrate) on Axum. Postgres with pgvector for the read model and
vector storage. Ollama and Cloudflare Workers AI for inference. Cloudflare
Vectorize is supported as an alternative vector store.
