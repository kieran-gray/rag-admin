# Chunk evaluation

A Work-In-Progress local-only Leptos app for evaluating retrieval pipelines against your own
documents. Pull source documents in, run different chunking strategies and
embedding models against them, generate synthetic Q&A datasets from the content,
then score retrieval quality across pipeline variants.

Not deployed. Run it locally, point it at a content source and a vector store,
and iterate on what gives you the best recall/precision before promoting a
pipeline to production.

## Run

The recommended way to run the app is via Docker Compose, which brings up
Postgres (with pgvector), [Ollama](https://ollama.com/), and the app itself in
one go.

Prerequisites:

- [Docker](https://docs.docker.com/get-docker/) (with Compose)
- Optional: [just](https://github.com/casey/just) for convenience recipes
- Optional: [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html)
  if you have an NVIDIA GPU and want Ollama to use it. Without a GPU, remove the
  `nvidia` device reservation from the `ollama` service in `docker-compose.yml`.

Copy `.env.example` to `.env`, then start the stack with the `ollama` profile:

```sh
docker compose --profile ollama up -d
# or, with just:
just up
```

Then open `http://localhost:3000`.

### First run: model downloads

Bringing up the `ollama` profile starts Ollama and pulls two seeded models into
the `ollama-data` volume:

- `qwen3-embedding:0.6b` for embeddings
- `gemma3:12b-it-qat` for generation

The first run downloads these models and can take a while. **The defaults are
not suitable for all hardware** — `gemma3:12b-it-qat` needs roughly 12–16 GB of
RAM/VRAM to run comfortably. If your machine can't fit it, edit the
`ollama-pull-generation` service in `docker-compose.yml` to pull a smaller
variant such as `gemma3:4b-it-qat` or `gemma3:1b-it-qat` and add a matching
catalog entry through the UI.

Adjust `OLLAMA_MEMORY_RESERVATION` and `OLLAMA_MEMORY_LIMIT` in `.env` if Docker
does not have enough memory available for the generation model. Keep at least
20 GB free in Docker's volume store for the Ollama model blobs.

### Running llama-server instead

[llama-swap](https://github.com/mostlygeek/llama-swap) (fronting `llama-server`
from llama.cpp) is also defined in `docker-compose.yml`, behind the `llama`
profile. To use it instead of Ollama, bring it up with the profile flag and
point the app at it:

```sh
docker compose --profile llama up -d
# then add a llama-server catalog entry through the UI (or set LLAMA_SERVER_BASE_URL).
```

llama-swap loads two models on demand — `Qwen/Qwen3-Embedding-0.6B-GGUF` for
embeddings and `ggml-org/gemma-3-12b-it-GGUF` for generation — downloading each
GGUF from Hugging Face the first time it's invoked and caching it in the
`llama-models` volume. Edit `deploy/llama-swap/config.yaml` to pick different
quantizations or models; each entry's `-hf user/repo:quant` argument resolves
directly to a Hugging Face file.

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
| `OLLAMA_BASE_URL` | Ollama endpoint. Defaults to `http://localhost:11434` |
| `OLLAMA_MEMORY_RESERVATION`, `OLLAMA_MEMORY_LIMIT` | Docker Compose memory settings for the local Ollama service |
| `OLLAMA_CONTEXT_LENGTH` | Ollama generation context size. Defaults to `16384` |
| `LLAMA_SERVER_BASE_URL` | Optional. OpenAI-compatible endpoint (llama-swap or a bare llama-server). Defaults to `http://localhost:8080`. Only used with the `llama` profile |
| `LLAMA_SWAP_IMAGE` | llama-swap container image variant. Defaults to `ghcr.io/mostlygeek/llama-swap:cuda` |
| `LLAMA_SERVER_MEMORY_RESERVATION`, `LLAMA_SERVER_MEMORY_LIMIT` | Docker Compose memory settings for the llama-swap service |
| `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_API_TOKEN` | Required if using Cloudflare Workers AI / Vectorize |
| `CLOUDFLARE_KV_NAMESPACE_ID` | Required when `KV_BACKEND=cloudflare` |
| `KV_BACKEND` | `postgres` (default) or `cloudflare` |

Embedding models, generation models, vector indexes, and pipeline configurations
are managed at runtime through the UI and stored in Postgres.

## Stack

Leptos (SSR + hydrate) on Axum. Postgres with pgvector for the read model and
vector storage. Ollama, llama-server (via llama-swap), and Cloudflare Workers
AI are all selectable inference backends. Cloudflare Vectorize is supported as
an alternative vector store.
