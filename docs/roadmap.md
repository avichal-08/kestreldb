# Roadmap

## Done

- Data model with valid time and recorded time, and links from facts to source episodes
- Batch log in object storage with create-only puts, so concurrent writers can't overwrite each other
- Replay on open, `refresh()` for followers, isolated namespaces
- Merge by kind and name, exclusive facts, closing facts, `as_of` queries
- Recall seeded by embeddings and keywords, expanded through the graph

## Next: storage

Opening a namespace replays its whole log and `refresh()` lists the whole prefix. Both get slower and cost more as memory grows.

- [ ] Snapshot the graph at sequence `N` under `ns/<namespace>/snapshots/<N>/`
- [ ] `manifest.json` pointing at the current snapshot, updated with a conditional put
- [ ] Open from the snapshot and replay only batches after `N`
- [ ] List from the last applied sequence instead of the whole prefix
- [ ] Delete batches older than the snapshot, after a grace period for slow readers
- [ ] Binary batch format with a checksum instead of JSON

## Then: search

- [ ] Vector index per namespace: HNSW in memory first, an index stored next to snapshots later
- [ ] BM25 over names, summaries, facts and episodes
- [ ] Reciprocal rank fusion of vector, keyword and graph results, with an optional reranker hook
- [ ] Recency and use-count weighting
- [ ] Evals for recall: a fixed set of conversations and questions, scored on whether the right facts come back

## Then: server

- [ ] HTTP server for writes, recall, facts and history
- [ ] Router that hashes namespaces to nodes, evicts idle ones and adds read replicas
- [ ] Auth, per-namespace quotas and metrics

## Then: agents

- [ ] MCP server with `remember`, `recall`, `facts` and `forget` tools
- [ ] Ingest that extracts entities and facts from episodes with an LLM
- [ ] Entity matching beyond exact names, and contradiction checks
- [ ] Python and TypeScript clients
