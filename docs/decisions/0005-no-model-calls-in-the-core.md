# 0005. No model calls in the core

- Status: Accepted
- Date: 2026-11-08

## Context

Most of the value in agent memory comes from turning raw text into entities and facts, matching "Bob" and "Robert", and embedding everything for search. All of that needs model calls, which are slow, cost money, fail in their own ways and give different answers on different days.

If the storage engine made those calls itself, a commit could take seconds and fail because a provider was down, and tests would depend on a network and a model version.

## Decision

The engine stores and retrieves. It never calls a model. Callers pass in entities, facts and embeddings ready to store, and recall takes the query text and, if wanted, its embedding from the caller. Extraction, entity matching and embedding belong to the caller or to a separate ingest layer built on top.

## Alternatives considered

- **Extraction inside the write path**, so callers write raw text and get a graph. Easiest to use, but ties every write to a model's latency, cost and failures, and makes results depend on which model ran.
- **Pluggable model hooks in the engine.** Keeps one api, but still puts model calls inside commits and recall, and makes the engine responsible for retries, rate limits and caching.

## Consequences

- The engine is deterministic and testable without a network, and commit latency is one PUT.
- Using it well takes more work from the caller until an ingest layer exists. That's on the [roadmap](../roadmap.md).
- Embeddings from different models can't be compared. A namespace has to stick to one embedding model, or re-embed when it changes. The engine doesn't check this.
