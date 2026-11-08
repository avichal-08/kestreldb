# KestrelDB

I'm building a graph memory for AI agents that keeps all its data in object storage (S3, GCS, Azure or a local folder).

The idea is simple. Agents store what happened (episodes), the things they know about (entities), and facts that link those things (relations). When a fact stops being true I don't delete it, I close it, so you can ask both what's true now and what was true at some point in the past. Every fact points back to the episode it came from.

There are no servers holding state. Each agent gets its own namespace, which is just a log of batches in a bucket. Any machine can open a namespace by reading that log.

It's early. The core works and has tests. Snapshots, a vector index and a server are next. See [docs/DESIGN.md](docs/DESIGN.md) for how it works and what I plan to build.

## Example

```rust
use std::sync::Arc;

use kestreldb::{EntityInput, Memory, Recall, RelationInput};
use object_store::memory::InMemory;

#[tokio::main]
async fn main() -> kestreldb::Result<()> {
    // swap for s3, gcs or a local dir
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

`exclusive()` closes any older `lives_in` fact for the same person. `commit()` writes the whole batch as one object.

Run it with `cargo run --example quickstart`, and run the tests with `cargo test`.
