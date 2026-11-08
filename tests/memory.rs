use std::sync::Arc;

use kestreldb::{EntityInput, Error, Memory, Recall, RelationInput};
use object_store::ObjectStore;
use object_store::memory::InMemory;

fn store() -> Arc<dyn ObjectStore> {
    Arc::new(InMemory::new())
}

#[tokio::test]
async fn writes_survive_reopen() {
    let store = store();
    let mut mem = Memory::open(store.clone(), "agent-1").await.unwrap();

    let mut w = mem.write();
    let ep = w.episode("chat:1", "Alice said she is leading the billing migration.", 1_000);
    let alice = w.entity(EntityInput::new("person", "Alice").from_episode(ep));
    let billing = w.entity(EntityInput::new("project", "Billing migration"));
    w.relate(RelationInput::new(alice, "leads", billing, "Alice leads the billing migration").from_episode(ep))
        .unwrap();
    w.commit().await.unwrap();

    let reopened = Memory::open(store, "agent-1").await.unwrap();
    assert_eq!(reopened.next_seq(), 1);
    let a = reopened.find_entity("person", "alice").expect("case-insensitive lookup");
    assert_eq!(a.id, alice);
    let facts = reopened.facts(alice, None);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].to, billing);
    assert_eq!(reopened.episode(ep).unwrap().source, "chat:1");
}

#[tokio::test]
async fn upsert_dedupes_and_merges() {
    let mut mem = Memory::open(store(), "ns").await.unwrap();
    let mut w = mem.write();
    let a = w.entity(EntityInput::new("person", "Bob").attr("team", "infra"));
    w.commit().await.unwrap();

    let mut w = mem.write();
    let b = w.entity(EntityInput::new("person", " bob ").summary("SRE").attr("tz", "UTC"));
    w.commit().await.unwrap();

    assert_eq!(a, b);
    let bob = mem.entity(a).unwrap();
    assert_eq!(bob.summary, "SRE");
    assert_eq!(bob.attributes.len(), 2);
}

#[tokio::test]
async fn exclusive_facts_supersede_and_history_is_queryable() {
    let mut mem = Memory::open(store(), "ns").await.unwrap();
    let mut w = mem.write();
    let carol = w.entity(EntityInput::new("person", "Carol"));
    let paris = w.entity(EntityInput::new("city", "Paris"));
    let berlin = w.entity(EntityInput::new("city", "Berlin"));
    w.relate(RelationInput::new(carol, "lives_in", paris, "Carol lives in Paris").valid_from(100).exclusive())
        .unwrap();
    w.commit().await.unwrap();

    let mut w = mem.write();
    w.relate(RelationInput::new(carol, "lives_in", berlin, "Carol lives in Berlin").valid_from(200).exclusive())
        .unwrap();
    w.commit().await.unwrap();

    let now: Vec<_> = mem.facts(carol, None).into_iter().map(|r| r.to).collect();
    assert_eq!(now, vec![berlin]);
    let then: Vec<_> = mem.facts(carol, Some(150)).into_iter().map(|r| r.to).collect();
    assert_eq!(then, vec![paris]);
    assert!(mem.facts(carol, Some(50)).is_empty());
}

#[tokio::test]
async fn concurrent_writers_conflict_and_recover() {
    let store = store();
    let mut a = Memory::open(store.clone(), "shared").await.unwrap();
    let mut b = Memory::open(store.clone(), "shared").await.unwrap();

    let mut w = a.write();
    w.entity(EntityInput::new("person", "Dana"));
    w.commit().await.unwrap();

    let mut w = b.write();
    w.entity(EntityInput::new("person", "Eve"));
    match w.commit().await {
        Err(Error::Conflict { seq: 0 }) => {}
        other => panic!("expected conflict, got {other:?}"),
    }

    assert_eq!(b.refresh().await.unwrap(), 1);
    assert!(b.find_entity("person", "Dana").is_some());
    let mut w = b.write();
    w.entity(EntityInput::new("person", "Eve"));
    assert_eq!(w.commit().await.unwrap(), 1);

    a.refresh().await.unwrap();
    assert!(a.find_entity("person", "Eve").is_some());
}

#[tokio::test]
async fn recall_seeds_and_expands() {
    let mut mem = Memory::open(store(), "ns").await.unwrap();
    let mut w = mem.write();
    let ep = w.episode("chat:9", "Frank mentioned the Q3 launch slipped because of the auth service.", 5);
    let frank = w.entity(EntityInput::new("person", "Frank").embedding(vec![1.0, 0.0, 0.0]));
    let launch = w.entity(EntityInput::new("project", "Q3 launch").embedding(vec![0.0, 1.0, 0.0]));
    let auth = w.entity(EntityInput::new("service", "Auth service").embedding(vec![0.0, 0.0, 1.0]));
    let unrelated = w.entity(EntityInput::new("person", "Grace"));
    w.relate(RelationInput::new(frank, "works_on", launch, "Frank works on the Q3 launch").from_episode(ep))
        .unwrap();
    w.relate(RelationInput::new(launch, "blocked_by", auth, "The Q3 launch is blocked by the auth service").from_episode(ep))
        .unwrap();
    w.commit().await.unwrap();

    let r = mem.recall(&Recall::embedding(vec![1.0, 0.0, 0.0]).k(1).hops(2));
    let ids: Vec<_> = r.entities.iter().map(|e| e.item.id).collect();
    assert_eq!(ids[0], frank);
    assert!(ids.contains(&launch) && ids.contains(&auth));
    assert!(!ids.contains(&unrelated));
    assert_eq!(r.relations.len(), 2);
    assert_eq!(r.episodes.len(), 1);

    let r = mem.recall(&Recall::text("what is blocking the launch?").k(2).hops(0));
    let ids: Vec<_> = r.entities.iter().map(|e| e.item.id).collect();
    assert!(ids.contains(&launch));
}

#[tokio::test]
async fn namespaces_are_isolated() {
    let store = store();
    let mut a = Memory::open(store.clone(), "a").await.unwrap();
    let mut w = a.write();
    w.entity(EntityInput::new("person", "Heidi"));
    w.commit().await.unwrap();

    let b = Memory::open(store, "b").await.unwrap();
    assert_eq!(b.next_seq(), 0);
    assert!(b.find_entity("person", "Heidi").is_none());
}

#[tokio::test]
async fn local_filesystem_backend_fences_writers() {
    let dir = tempfile::tempdir().unwrap();
    let fs: Arc<dyn ObjectStore> = Arc::new(object_store::local::LocalFileSystem::new_with_prefix(dir.path()).unwrap());
    let mut a = Memory::open(fs.clone(), "disk").await.unwrap();
    let mut b = Memory::open(fs.clone(), "disk").await.unwrap();

    let mut w = a.write();
    w.entity(EntityInput::new("person", "Ivan"));
    w.commit().await.unwrap();

    let mut w = b.write();
    w.entity(EntityInput::new("person", "Judy"));
    assert!(matches!(w.commit().await, Err(Error::Conflict { .. })));

    let reopened = Memory::open(fs, "disk").await.unwrap();
    assert!(reopened.find_entity("person", "Ivan").is_some());
    assert!(reopened.find_entity("person", "Judy").is_none());
}
