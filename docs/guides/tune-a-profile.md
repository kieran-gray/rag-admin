---
title: Tune a profile
summary: Adjust chunking, embedding, and retrieval settings to improve recall.
section: Tuning
order: 1
---

# Tune a profile

Profiles hold the settings that control retrieval. There are two kinds:

- **Index profile** — how documents are chunked, which embedding model is used, and which vector index receives the results.
- **Retrieval profile** — how a query is run: top-k, score threshold, and reranking.

You manage both under **Pipeline → Profiles**.

## What to adjust

- **Chunking strategy** — smaller chunks improve precision but can lose context; larger chunks keep context but dilute relevance. Try a different strategy under **Pipeline → Chunking**.
- **Embedding model** — a stronger model usually improves recall at some cost in speed.
- **Top-k and threshold** — raising top-k retrieves more candidates; a score threshold drops weak matches.
- **Reranking** — a reranker reorders candidates and often lifts recall@k noticeably.

## Tune with evidence

1. Run an [evaluation](/docs/run-an-evaluation) to get a baseline.
2. Change one setting.
3. Re-run and compare. Keep the change if recall improves.

Changing one thing at a time keeps the comparison honest. When you find a good combination, set it as the default so new documents use it automatically.
