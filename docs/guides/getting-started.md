---
title: Getting started
summary: What rag-admin does and how the pieces fit together.
section: Getting started
order: 1
---

# Getting started

rag-admin helps you build a retrieval pipeline and measure how well it works. You add documents, index them, then evaluate and tune the settings that control retrieval quality.

## The main areas

- **Documents** — add files or URLs and index them into a vector store.
- **Connectors** — pull documents in bulk from an external source.
- **Evaluation** — run question sets against your index and track recall across variants.
- **Playground** — try retrieval, embeddings, and grounded chat by hand.
- **Pipeline** — manage the profiles, chunking strategies, models, and indexes that the rest of the app uses.

## A typical first run

1. Add a document or two from the **Documents** page.
2. Index them with an index profile.
3. Open the **Playground** and run a query to see what comes back.
4. Create an evaluation dataset and run it to get a recall score.
5. Adjust a profile and re-run to compare.

Each area has its own guide. Start with [adding and indexing documents](/docs/add-and-index-documents).
