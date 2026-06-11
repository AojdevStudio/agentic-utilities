# How Semantic Cache Invalidation Works

This note explains the cache invalidation model used by the answer search service. The important idea is that a cache entry is only reusable when both the query meaning and the source corpus version still match.

## TL;DR

The cache key is not just the user's raw query. It combines the normalized embedding, retrieval filters, model family, and corpus watermark so stale answers are avoided when source documents change.

## Request Path

1. Normalize the user query and compute an embedding.
2. Search for near-neighbor cache entries inside the same tenant and filter set.
3. Reject candidates whose corpus watermark is older than the latest indexed source.
4. Re-rank remaining candidates by semantic distance and answer shape.

## Configuration Matrix

| Setting | Default | Why It Matters |
| --- | --- | --- |
| `max_distance` | `0.08` | Keeps unrelated queries from sharing answers |
| `watermark_mode` | `strict` | Prevents stale answers after source updates |
| `model_family` | `same-family` | Avoids mixing answer formats across models |

## Glossary

- **Corpus watermark:** The highest source version included in an index build.
- **Semantic distance:** How far apart two normalized embeddings are.
- **Answer shape:** The expected response form, such as summary, table, or checklist.

## FAQ

### Why not invalidate everything on every document change?

Full invalidation is simple but expensive. Watermarks let unrelated tenants and unchanged source slices keep useful cache entries.

### What happens when the embedding model changes?

The model family becomes part of the cache boundary. Old entries age out instead of being reused across incompatible vectors.
