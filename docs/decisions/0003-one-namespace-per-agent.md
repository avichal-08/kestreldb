# 0003. One namespace per agent, not one sharded graph

- Status: Accepted
- Date: 2026-11-08

## Context

"Distributed graph database" usually means one large graph split across machines, with queries that hop between them. That's the hard version: partitioning to keep neighbours together, traversals across the network, and transactions that span shards.

Agent memory doesn't usually look like that. Memory belongs to an agent, a user or a tenant, and recall almost always stays inside it. Each memory is small, from thousands to a few million records, and there are many of them.

## Decision

The unit of data and of distribution is the namespace. A namespace is a separate log with its own sequence numbers, and nothing crosses between namespaces: no shared entities, no cross-namespace facts, no transactions over two of them.

Scaling out means spreading namespaces over nodes. A router hashes each namespace to a node so its cache stays warm, and any node can take a namespace over by replaying its log.

## Alternatives considered

- **One graph partitioned across nodes.** Needed for a single huge shared graph, like a company-wide knowledge graph, but it brings distributed traversals and cross-shard writes for a workload that rarely needs them.
- **One graph with a tenant id on every record.** Simple to start, but every query has to filter by tenant, one tenant's history affects everyone's open and compaction time, and deleting a tenant means rewriting shared data.

## Consequences

- A traversal never leaves one machine, and a namespace can be opened, moved, copied or deleted as one prefix.
- One namespace is limited to what one node can hold and serve. A memory that outgrows that needs splitting by the caller or a later design.
- Knowledge shared between agents has to be copied into each namespace or kept in a namespace of its own that agents query separately.
