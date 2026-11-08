# Design

These are my notes on how KestrelDB works and where I want to take it.

## What I'm building

A memory layer for AI agents. It isn't a general-purpose graph database. I care about what agents actually do: save what happened, pull facts out of it, recall the right context later, and keep up when facts change.

A few things I want to hold on to:

- Object storage is the only place data lives. No disks to manage, no replication to write. S3 already handles durability.
- Each agent, user or tenant gets its own namespace. Idle namespaces should cost close to nothing.
- Facts are closed, not deleted. I want to answer "what's true now" and "what was true then", and trace every fact back to where it came from.

What I'm not doing for now: Cypher or Neo4j compatibility, transactions across namespaces, and analytics over one giant graph.

## Data model

There are three kinds of records.

- Episode: raw input, like a chat turn, a chunk of a document or a tool result. It never changes.
- Entity: a node, like a person or a project. Within a namespace it's unique by kind and name. Writing the same one again merges into it.
- Relation: a directed edge between two entities with a type (`lives_in`) and the fact in plain words ("Carol lives in Berlin"). It links back to the episodes it came from.

Entities and relations can carry an embedding.

Every relation tracks two kinds of time. Valid time (`valid_from`, `valid_to`) is when the fact was true in the world. Transaction time (`created_at`, `invalidated_at`) is when I learned it and when I learned it stopped being true.

If a relation is marked exclusive, writing it closes any older open fact with the same subject and type. Someone has one current city, so a new `lives_in` ends the old one.

## Storage

Each namespace is a log of batches:

```
ns/<namespace>/wal/00000000000000000000.json
ns/<namespace>/wal/00000000000000000001.json
...
```

Later I'll add snapshots under `ns/<namespace>/snapshots/` and a `manifest.json` that points at the latest one.

### Writes

You collect changes in a `Writer` and commit them as one batch. The batch goes to `wal/<seq>` with a create-only put (`If-None-Match: *` on S3; GCS and Azure have the same thing). If that object already exists, someone else got there first. The writer gets `Error::Conflict`, calls `refresh()` and tries again.

That gives me one writer per namespace with no lock service or leader election. A batch is all or nothing.

The cost is that each commit is one S3 PUT, roughly 10 to 100 ms. For agent memory that's fine, since the LLM step that produces the facts takes seconds anyway. It does mean writes should be batched.

### Reads

Opening a namespace lists `wal/` and replays every batch into an in-memory graph. `refresh()` only reads the batches after the last one applied, so a reader can follow a namespace cheaply.

Right now opening means replaying the whole log. Snapshots will fix that.

## Distribution

I split work by namespace, not by cutting one graph across machines.

- Nodes keep no state. Any node can serve any namespace. If one dies, you lose its cache and nothing else.
- A small router picks a node for each namespace with consistent hashing so caches stay warm. If a node goes away, another one picks the namespace up and replays it.
- The router normally sends writes for a namespace to one node, but correctness doesn't depend on that. The create-only put blocks a stale or duplicate writer either way.
- A busy namespace can have several readers, each calling `refresh()`.
- Data is cached in RAM, then on local disk, then read from the bucket. Cold namespaces get evicted.

I'm leaving out sharding a single huge graph until I actually need it. Most agent memories are small, and keeping a namespace on one node means traversals never cross the network.

## Recall

1. Find starting points. Score entities and facts against the query by embedding similarity and word overlap. A matching fact counts for both of its ends. Keep the top `k`.
2. Walk the graph `hops` steps out from there, following only facts that hold at `as_of`. Each hop halves the score.
3. Return the ranked entities, the facts between them, and the episodes behind those facts, newest first. That's ready to drop into a prompt.

Finding starting points is a full scan for now. That's okay for small namespaces and it's the first thing I'll replace.

## LLMs

The database doesn't call any model. It stores embeddings and facts, nothing more. Pulling facts out of text, matching entities beyond exact names, and making embeddings will live in a separate ingest layer. That keeps the storage engine predictable and easy to test.

## Plan

Done:

- [x] Data model with two kinds of time and links back to episodes
- [x] Log in object storage with create-only puts
- [x] Replay, `refresh()` and separate namespaces
- [x] Merge by name, exclusive facts, closing facts, `as_of` queries
- [x] Recall by embedding and keywords, then graph walk

Storage:

- [ ] Snapshots that compact the log into segment files, plus a manifest updated with a conditional put
- [ ] Drop old log entries after a snapshot
- [ ] Binary batch format with checksums instead of JSON
- [ ] Local disk cache for segments

Search:

- [ ] Vector index per namespace (HNSW in memory, a disk-friendly index on object storage later)
- [ ] BM25 over names, summaries, facts and episodes
- [ ] Merge vector, text and graph results with reciprocal rank fusion, plus an optional reranker
- [ ] Weight by recency and how often something is used

Server:

- [ ] HTTP or gRPC server for writes, recall, facts and history
- [ ] Router with consistent hashing, idle eviction and read replicas
- [ ] Auth, quotas and metrics

Agent side:

- [ ] Ingest that uses an LLM to pull entities and facts out of episodes
- [ ] Entity matching and contradiction checks
- [ ] MCP server so agents can use it as a memory tool
- [ ] Python and TypeScript clients
