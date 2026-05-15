# rag-admin

A Work-In-Progress local-only Leptos app for evaluating retrieval pipelines against your own
documents. Pull source documents in, run different chunking strategies and
embedding models against them, generate synthetic Q&A datasets from the content,
then score retrieval quality across pipeline variants.

Not deployed. Run it locally, point it at a content source and a vector store,
and iterate on what gives you the best recall/precision before promoting a
pipeline to production.

## Run

You need Postgres (with the `vector` extension) and, optionally, Ollama or
Cloudflare credentials depending on which providers you want to use.

```sh
docker compose up -d db
cargo install cargo-leptos
cargo leptos watch
```

Then open `http://127.0.0.1:3000`.

To run the app and local Ollama stack fully in Compose:

```sh
docker compose up -d
```

The Compose stack starts Ollama, stores its model files in the `ollama-data`
volume, and populates the local models seeded by the app:

- `qwen3-embedding:0.6b` for embeddings
- `ministral-3:14b` for generation

The first run downloads the model files and can take a while. Adjust
`OLLAMA_MEMORY_RESERVATION` and `OLLAMA_MEMORY_LIMIT` in `.env` if Docker does
not have enough memory available for the generation model. Keep at least 20 GB
free in Docker's volume store for the Ollama model blobs, partial downloads, and
future model updates.

## Configuration

Environment variables (see `.env.example`):

| Variable | Purpose |
| --- | --- |
| `DATABASE_URL` | Postgres connection string (pgvector required) |
| `BLOG_URL` | Source-document adapter base URL |
| `OLLAMA_BASE_URL` | Optional. Defaults to `http://localhost:11434` |
| `OLLAMA_MEMORY_RESERVATION`, `OLLAMA_MEMORY_LIMIT` | Docker Compose memory settings for the local Ollama service |
| `OLLAMA_NUM_CTX` | Ollama generation context size. Defaults to `16384` |
| `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN` | Required if using Cloudflare Workers AI / Vectorize |
| `CLOUDFLARE_KV_NAMESPACE_ID` | Required when `KV_BACKEND=cloudflare` |
| `KV_BACKEND` | `postgres` (default) or `cloudflare` |

Embedding models, generation models, vector indexes, and pipeline configurations
are managed at runtime through the UI and stored in Postgres.

## Stack

Leptos (SSR + hydrate) on Axum. Postgres with pgvector for the read model and
vector storage. Ollama and Cloudflare Workers AI for inference. Cloudflare
Vectorize is supported as an alternative vector store.
