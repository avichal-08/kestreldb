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
