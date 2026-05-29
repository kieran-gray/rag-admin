---
title: Run an evaluation
summary: Measure retrieval quality with a question set and compare variants.
section: Workflows
order: 2
---

# Run an evaluation

An evaluation measures how often the right content is retrieved for a set of questions. It gives you a score you can track as you change settings.

## Build a dataset

A dataset is a set of questions, each tied to the content that should answer it.

1. Go to **Evaluation → Datasets** and create a dataset.
2. Add questions. Each question references the chunks or documents that count as correct.

You can also generate questions from a comprehension map if you have one.

## Run it

1. From **Evaluation → Runs**, start a run against your dataset.
2. Pick the profiles or variants you want to compare.
3. When the run finishes, review recall@k and the per-question breakdown.

## Read the results

- **Recall@k** tells you how often a correct chunk appeared in the top *k* results.
- The per-question view shows exactly what was retrieved, so you can see *why* a question scored low.

Use the comparison to decide which variant to keep, then [tune the winning profile](/docs/tune-a-profile) further.
