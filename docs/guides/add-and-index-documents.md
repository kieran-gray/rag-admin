---
title: Add & index documents
summary: Bring content in from files or URLs and index it for retrieval.
section: Workflows
order: 1
---

# Add & index documents

Everything you want to retrieve over lives in the **Documents** list. A document can come from an uploaded file, a URL, or a connector.

## Add a document

1. Go to **Documents** and choose **Add**.
2. Pick a source:
   - **File** — upload `.md`, `.txt`, `.html`, or `.pdf`.
   - **URL** — the server fetches the page and converts it to Markdown, stripping navigation and other boilerplate.
3. Confirm, and the document appears in the list.

## Index it

Indexing turns a document into retrievable chunks:

1. Open the document and start indexing.
2. Choose an **index profile**. The profile decides how the document is chunked, which embedding model is used, and which vector index the results go into.
3. Watch the activity tray for progress. When it finishes, the document is searchable.

## Check the result

Open the **Playground** and run a query. If the chunks that come back look wrong — too long, too short, or missing context — that's a signal to [tune the profile](/docs/tune-a-profile).
