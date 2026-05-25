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
Postgres (with pgvector), [llama-swap](https://github.com/mostlygeek/llama-swap)
(fronting `llama-server` from llama.cpp), and the app itself in one go.

Prerequisites:

- [Docker](https://docs.docker.com/get-docker/) (with Compose)
- Optional: [just](https://github.com/casey/just) for convenience recipes
- Optional: [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html)
  if you have an NVIDIA GPU. Without a GPU, override `LLAMA_SWAP_IMAGE` in
  `.env` to the CPU variant (`ghcr.io/mostlygeek/llama-swap:cpu`) and remove
  the `nvidia` device reservation from `docker-compose.yml`.

Copy `.env.example` to `.env`, then start the stack:

```sh
docker compose up -d
# or, with just:
just up
```

Then open `http://localhost:3000`.

### First run: model downloads

The Compose stack starts llama-swap with two seeded models loaded on demand:

- `qwen3-embedding-0.6b` for embeddings (`Qwen/Qwen3-Embedding-0.6B-GGUF`)
- `gemma3-12b` for generation (`ggml-org/gemma-3-12b-it-GGUF`)

llama-server downloads each GGUF from Hugging Face the first time it's
invoked and caches it in the `llama-models` volume. To pick different
quantizations or models, edit `deploy/llama-swap/config.yaml` — each entry's
`-hf user/repo:quant` argument resolves directly to a Hugging Face file.

**The defaults are not suitable for all hardware** — the Q4_K_M quantization of
`gemma3-12b` needs roughly 12–16 GB of RAM/VRAM to run comfortably. If your
machine can't fit it, pick a smaller variant (e.g. `Q3_K_M`) or a smaller model
in `deploy/llama-swap/config.yaml` and add a matching catalog entry through
the UI.

Adjust `LLAMA_SERVER_MEMORY_RESERVATION` and `LLAMA_SERVER_MEMORY_LIMIT` in
`.env` if Docker does not have enough memory available. Keep at least 20 GB
free in Docker's volume store for the model files.

### Running Ollama instead

The Ollama services are still defined in `docker-compose.yml` but live behind
the `ollama` profile and are not started by default. To use Ollama instead of
llama-server, bring it up with the profile flag and point the app at it:

```sh
docker compose --profile ollama up -d ollama ollama-pull-embedding ollama-pull-generation
# then add an Ollama catalog entry through the UI (or set OLLAMA_BASE_URL).
```

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
| `LLAMA_SERVER_BASE_URL` | OpenAI-compatible endpoint (llama-swap or a bare llama-server). Defaults to `http://localhost:8080` |
| `LLAMA_SWAP_IMAGE` | llama-swap container image variant. Defaults to `ghcr.io/mostlygeek/llama-swap:cuda` |
| `LLAMA_SERVER_MEMORY_RESERVATION`, `LLAMA_SERVER_MEMORY_LIMIT` | Docker Compose memory settings for the llama-swap service |
| `OLLAMA_BASE_URL` | Optional. Defaults to `http://localhost:11434`. Only used when an Ollama catalog entry is selected |
| `OLLAMA_MEMORY_RESERVATION`, `OLLAMA_MEMORY_LIMIT` | Docker Compose memory settings for the local Ollama service (Ollama profile only) |
| `OLLAMA_CONTEXT_LENGTH` | Ollama generation context size. Defaults to `16384` |
| `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN` | Required if using Cloudflare Workers AI / Vectorize |
| `CLOUDFLARE_KV_NAMESPACE_ID` | Required when `KV_BACKEND=cloudflare` |
| `KV_BACKEND` | `postgres` (default) or `cloudflare` |

Embedding models, generation models, vector indexes, and pipeline configurations
are managed at runtime through the UI and stored in Postgres.

## Stack

Leptos (SSR + hydrate) on Axum. Postgres with pgvector for the read model and
vector storage. llama-server (via llama-swap), Ollama, and Cloudflare Workers
AI are all selectable inference backends. Cloudflare Vectorize is supported as
an alternative vector store.
