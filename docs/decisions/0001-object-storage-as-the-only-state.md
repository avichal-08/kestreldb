# 0001. Object storage is the only place state lives

- Status: Accepted
- Date: 2026-11-08

## Context

Agent memory is many small, mostly idle datasets. One user's agent might write a few batches a day and read its memory a few times an hour. A database that keeps every dataset on a running server pays for disks, replicas and failover around the clock, for data that's rarely touched.

Object storage already gives durability across zones, effectively unlimited space and a low price per gigabyte. Since S3 added conditional writes in 2024, it can also refuse a write when the object already exists, which is enough to order commits (see [0002](0002-create-only-puts-instead-of-a-lock-service.md)).

## Decision

The bucket is the database. A namespace is a prefix, every commit is an object, and processes that serve memory keep only caches. Any process with access to the bucket can open any namespace by reading its log.

The engine talks to storage through the `object_store` crate, so the same code runs on S3, GCS, Azure, R2, MinIO, a local directory and in memory for tests.

## Alternatives considered

- **Embedded storage on local disk** (RocksDB or SQLite per namespace). Fast reads and writes, but the node owning the disk becomes the only one that can serve the namespace, so durability and failover have to be built on top.
- **A managed database** (Postgres with pgvector, or a graph database). Mature and queryable, but always on, and per-tenant isolation is either a schema per tenant or a tenant column on every table.
- **Object storage with a database for metadata.** Common in lakehouse designs. It adds a second system to run and keep consistent, when conditional puts already cover what the metadata store would do here.

## Consequences

- Every commit is a PUT, about 10 to 100 ms on S3. Writes have to be batched, and anything that needs sub-millisecond writes isn't a fit.
- Opening a namespace means reading objects. Without snapshots, open time and cost grow with history, which is why snapshots are first on the [roadmap](../roadmap.md).
- Request costs, not storage, dominate the bill for busy namespaces, so listing and polling have to stay cheap.
- An idle namespace costs only its bytes in the bucket.
