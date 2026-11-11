# kestreldb

Graph memory for AI agents, stored in object storage. Agents write what happened, the things they know about and the facts that link them, then recall the relevant part of that graph as context for the next prompt.

Facts are closed instead of deleted, so memory can answer both "where does the user live now" and "where did they live in March", and every fact points back to the episode it was learned from. The only state is a bucket: each agent gets a namespace, a namespace is a log of batches, and any machine can open it by reading the log.

## Stack

- **language**: Rust (edition 2024), async on Tokio
- **storage**: anything [`object_store`](https://docs.rs/object_store) speaks: S3, GCS, Azure, R2, MinIO, a local directory or memory
- **format**: one JSON object per committed batch, for now

## Layout

```
src/model.rs            episodes, entities, relations, ops
src/log.rs              batch log in object storage, create-only puts
src/graph.rs            in-memory graph rebuilt from the log
src/memory.rs           public api: open, write, refresh, facts, recall
tests/memory.rs         end-to-end tests on in-memory and local-disk stores
examples/quickstart.rs  the example below
docs/                   architecture, decisions and roadmap
```

## Quickstart

Requires Rust 1.88 or newer.

```bash
cargo test
cargo run --example quickstart
```

```rust
use std::sync::Arc;

use kestreldb::{EntityInput, Memory, Recall, RelationInput};
use object_store::memory::InMemory;

#[tokio::main]
async fn main() -> kestreldb::Result<()> {
    let store = Arc::new(InMemory::new());
    let mut mem = Memory::open(store, "agent-42").await?;

    let mut w = mem.write();
    let ep = w.episode("chat:7", "I moved from Paris to Berlin last month.", 1_700_000_000_000);
    let user = w.entity(EntityInput::new("person", "User"));
    let berlin = w.entity(EntityInput::new("city", "Berlin"));
    w.relate(
        RelationInput::new(user, "lives_in", berlin, "The user lives in Berlin")
            .from_episode(ep)
            .exclusive(),
    )?;
    w.commit().await?;

    let ctx = mem.recall(&Recall::text("where does the user live?").hops(1));
    for fact in &ctx.relations {
        println!("fact: {}", fact.fact);
    }
    for ep in &ctx.episodes {
        println!("source [{}]: {}", ep.source, ep.content);
    }
    Ok(())
}
```

`exclusive()` closes any older open `lives_in` fact for the same person. `commit()` writes the whole batch as one object, so it lands completely or not at all.

### On S3

Enable the `aws` feature of `object_store` in your crate and pass the store in. Credentials and region come from the usual `AWS_*` variables.

```toml
object_store = { version = "0.14", features = ["aws"] }
```

```rust
use object_store::aws::AmazonS3Builder;

let store = AmazonS3Builder::from_env()
    .with_bucket_name("my-agent-memory")
    .build()?;
let mut mem = Memory::open(Arc::new(store), "agent-42").await?;
```

Commits rely on create-only puts (`If-None-Match: *`). S3, R2 and MinIO support them, and `object_store` sends them by default. Don't set `conditional_put` to `disabled`, or every commit fails.

## API

| Call | What it does |
| --- | --- |
| `Memory::open(store, ns)` | replay a namespace's log into memory |
| `mem.write()` | start a batch, returns a `Writer` |
| `w.episode(source, text, at)` | record raw input |
| `w.entity(EntityInput)` | insert, or merge into the entity with the same kind and name |
| `w.relate(RelationInput)` | add a fact between two entities, optionally `exclusive()` |
| `w.invalidate(id, at)` | close a fact from `at` onwards |
| `w.commit()` | write the batch, returns its sequence number |
| `mem.refresh()` | apply batches other writers committed since the last read |
| `mem.facts(id, as_of)` | facts touching an entity, now or at a point in time |
| `mem.recall(&Recall)` | ranked entities, the facts between them and their source episodes |

If another writer committed first, `commit()` returns `Error::Conflict`. Call `refresh()`, rebuild the batch against the new state and commit again.

## How it works

`write → batch → create-only put to wal/<seq> → replay on open, tail on refresh → recall`

| Doc | What it covers |
| --- | --- |
| [docs/architecture.md](docs/architecture.md) | data model, storage layout, write and read paths, recall, distribution |
| [docs/decisions](docs/decisions/README.md) | why the big choices were made |
| [docs/roadmap.md](docs/roadmap.md) | what's next |

## Status

The core engine works and is tested: writes with merges and exclusive facts, history queries, writer conflicts across processes, recall, and namespaces on in-memory and local-disk stores.

Not there yet: opening a namespace replays its whole log, every `refresh()` lists the whole log, and recall scans every entity and fact. Next is snapshots with a manifest, then a vector and keyword index, then a server. See [docs/roadmap.md](docs/roadmap.md).
