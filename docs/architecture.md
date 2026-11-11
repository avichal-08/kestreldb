# Architecture

kestreldb keeps an agent's memory as a graph: episodes it saw, entities it knows about and facts between them. The graph is rebuilt in memory from a log of batches in object storage. Nothing else holds state.

## Components

| Part | Role |
| --- | --- |
| `src/model.rs` | records (`Episode`, `Entity`, `Relation`) and the ops that change them |
| `src/log.rs` | appends and reads batches under `ns/<namespace>/wal/` |
| `src/graph.rs` | the in-memory graph: maps by id, adjacency, a name index |
| `src/memory.rs` | `Memory` and `Writer`, the public api, and recall |

## Data model

- **Episode**: raw input, like a chat turn, a document chunk or a tool result. Has a `source`, the text and when it happened. Never changes.
- **Entity**: a node with a `kind` (`person`, `project`, ...), a `name`, a summary, free-form attributes and an optional embedding. Unique per namespace by kind and name, compared trimmed and lowercased. Writing it again merges: a new summary or embedding replaces the old one, attributes are merged key by key, and source episodes accumulate.
- **Relation**: a directed edge with a `predicate` (`lives_in`) and the fact in words ("Carol lives in Berlin"), an optional embedding and the episodes it came from.

Relations carry two kinds of time:

| Field | Meaning |
| --- | --- |
| `valid_from`, `valid_to` | when the fact held in the world; `None` means open-ended |
| `created_at`, `invalidated_at` | when memory recorded it and when it was closed |

A relation is current when it has neither `valid_to` nor `invalidated_at`. A query with `as_of: t` uses valid time only: the fact counts if `valid_from <= t < valid_to`. Closing a fact never removes it (see [0004](decisions/0004-close-facts-instead-of-deleting.md)).

`exclusive()` closes every current fact with the same subject and predicate when the new one is written, with `valid_to` set to the new fact's `valid_from`, or to the time the batch was started if there's none. It only looks at committed facts, not ones staged earlier in the same batch.

## Storage

```
ns/<namespace>/wal/00000000000000000000.json   batch 0
ns/<namespace>/wal/00000000000000000001.json   batch 1
...
```

Each object is one `Batch { seq, committed_at, ops }`. Sequence numbers are zero-padded to 20 digits so listing order is commit order. A namespace can't be empty or contain `/`.

## Write path

```
writer.entity / relate / episode / invalidate  ──►  staged ops
commit  ──►  PUT wal/<next_seq>.json  (If-None-Match: *)
        ├─ created         ──►  apply to the local graph, next_seq += 1
        └─ already exists  ──►  Error::Conflict, nothing applied
```

- **Staging.** The `Writer` resolves merges against the committed graph and against entities staged earlier in the batch, so the same name twice in one batch gives one entity.
- **Commit.** One object per batch, written with a create-only put. If two writers race for the same sequence number, exactly one put succeeds. The other gets `Error::Conflict` and has to `refresh()` and rebuild its batch, since merges and exclusive facts depend on the state it missed. No lock service or leader is needed (see [0002](decisions/0002-create-only-puts-instead-of-a-lock-service.md)).
- **Cost.** One PUT per commit, so callers should batch. For agent memory, writes usually come after an LLM call that takes seconds, so the PUT is not the bottleneck.

## Read path

```
open      ──►  LIST wal/  ──►  GET every batch in order  ──►  apply
refresh   ──►  LIST wal/  ──►  GET batches >= next_seq   ──►  apply
```

Replay checks the log has no gaps and that each object holds the sequence number its name says. A gap or mismatch is `Error::Corrupt`.

Two known costs: `open` reads the whole log, and `refresh` lists the whole prefix even when nothing is new. Snapshots and listing from an offset fix both and are the next step in the [roadmap](roadmap.md).

## Recall

```
query (text and/or embedding)
  ──►  score every entity (name + summary) and every current fact
  ──►  top k seeds
  ──►  walk `hops` steps over facts active at as_of, score halves per hop
  ──►  entities, facts between them, source episodes newest first
```

- **Scoring.** Cosine similarity against the stored embedding, clamped at zero, plus the share of query words (two or more letters, lowercased) found in the text. A matching fact seeds both of its ends.
- **Walk.** Breadth-first from the seeds. An entity keeps the best score it's reached with, and the walk stops after any hop that brings in `max_entities` or more.
- **Result.** Up to `max_entities` entities by score, every fact whose two ends both made it in, and up to `max_episodes` source episodes, newest first.

Defaults are `k = 5`, `hops = 1`, `max_entities = 50` and `max_episodes = 20`. Scoring is a full scan for now, fine for small namespaces and the first thing to replace with an index.

## Distribution

The unit of distribution is the namespace, not a slice of one big graph (see [0003](decisions/0003-one-namespace-per-agent.md)).

- **Stateless nodes.** Any process can open any namespace. Losing a node loses its cache, nothing else.
- **Routing.** A router hashes namespaces to nodes so caches stay warm. If a node goes away, the next one replays the namespace.
- **Writers.** The router normally sends a namespace's writes to one node, but correctness doesn't depend on it. A stale or duplicate writer gets a conflict either way.
- **Readers.** A busy namespace can have several readers, each following the log with `refresh()`.

None of the routing exists yet. Today kestreldb is a library, and the conflict handling is what makes running it on several machines safe.

## Models

The engine never calls a model. Embeddings come in with the data, and extracting entities and facts from text belongs to the caller or to a separate ingest layer (see [0005](decisions/0005-no-model-calls-in-the-core.md)).
