use std::collections::BTreeMap;
use std::sync::Arc;

use object_store::ObjectStore;

use crate::graph::{Graph, name_key};
use crate::log::{Batch, Log};
use crate::model::*;
use crate::{Error, Result};

pub struct Memory {
    namespace: String,
    log: Log,
    next_seq: u64,
    graph: Graph,
}

impl Memory {
    pub async fn open(store: Arc<dyn ObjectStore>, namespace: impl Into<String>) -> Result<Self> {
        let namespace = namespace.into();
        if namespace.is_empty() || namespace.contains('/') {
            return Err(Error::InvalidInput(format!("bad namespace {namespace:?}")));
        }
        let mut mem = Self {
            log: Log::new(store, &namespace),
            namespace,
            next_seq: 0,
            graph: Graph::default(),
        };
        mem.refresh().await?;
        Ok(mem)
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    pub async fn refresh(&mut self) -> Result<usize> {
        let batches = self.log.read_from(self.next_seq).await?;
        let n = batches.len();
        for batch in batches {
            self.apply(&batch);
        }
        Ok(n)
    }

    fn apply(&mut self, batch: &Batch) {
        for op in &batch.ops {
            self.graph.apply(op);
        }
        self.next_seq = batch.seq + 1;
    }

    pub async fn commit(&mut self, ops: Vec<Op>) -> Result<u64> {
        let batch = Batch {
            seq: self.next_seq,
            committed_at: now(),
            ops,
        };
        self.log.append(&batch).await?;
        self.apply(&batch);
        Ok(batch.seq)
    }

    pub fn write(&mut self) -> Writer<'_> {
        Writer {
            now: now(),
            mem: self,
            ops: Vec::new(),
            staged: BTreeMap::new(),
        }
    }

    pub fn entity(&self, id: EntityId) -> Option<&Entity> {
        self.graph.entities.get(&id)
    }

    pub fn find_entity(&self, kind: &str, name: &str) -> Option<&Entity> {
        self.graph
            .by_name
            .get(&name_key(kind, name))
            .and_then(|id| self.graph.entities.get(id))
    }

    pub fn relation(&self, id: RelationId) -> Option<&Relation> {
        self.graph.relations.get(&id)
    }

    pub fn episode(&self, id: EpisodeId) -> Option<&Episode> {
        self.graph.episodes.get(&id)
    }

    pub fn facts(&self, id: EntityId, as_of: Option<Timestamp>) -> Vec<&Relation> {
        let mut facts: Vec<_> = self.graph.edges(id, as_of).collect();
        facts.sort_by_key(|r| r.created_at);
        facts
    }

}

#[derive(Debug, Clone, Default)]
pub struct EntityInput {
    pub kind: String,
    pub name: String,
    pub summary: Option<String>,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub embedding: Option<Vec<f32>>,
    pub episode: Option<EpisodeId>,
}

impl EntityInput {
    pub fn new(kind: impl Into<String>, name: impl Into<String>) -> Self {
        Self { kind: kind.into(), name: name.into(), ..Default::default() }
    }
    pub fn summary(mut self, s: impl Into<String>) -> Self {
        self.summary = Some(s.into());
        self
    }
    pub fn attr(mut self, k: impl Into<String>, v: impl Into<serde_json::Value>) -> Self {
        self.attributes.insert(k.into(), v.into());
        self
    }
    pub fn embedding(mut self, e: Vec<f32>) -> Self {
        self.embedding = Some(e);
        self
    }
    pub fn from_episode(mut self, id: EpisodeId) -> Self {
        self.episode = Some(id);
        self
    }
}

#[derive(Debug, Clone)]
pub struct RelationInput {
    pub from: EntityId,
    pub predicate: String,
    pub to: EntityId,
    pub fact: String,
    pub embedding: Option<Vec<f32>>,
    pub valid_from: Option<Timestamp>,
    pub episode: Option<EpisodeId>,
    // closes earlier active facts with the same from + predicate
    pub exclusive: bool,
}

impl RelationInput {
    pub fn new(from: EntityId, predicate: impl Into<String>, to: EntityId, fact: impl Into<String>) -> Self {
        Self {
            from,
            predicate: predicate.into(),
            to,
            fact: fact.into(),
            embedding: None,
            valid_from: None,
            episode: None,
            exclusive: false,
        }
    }
    pub fn valid_from(mut self, t: Timestamp) -> Self {
        self.valid_from = Some(t);
        self
    }
    pub fn embedding(mut self, e: Vec<f32>) -> Self {
        self.embedding = Some(e);
        self
    }
    pub fn from_episode(mut self, id: EpisodeId) -> Self {
        self.episode = Some(id);
        self
    }
    pub fn exclusive(mut self) -> Self {
        self.exclusive = true;
        self
    }
}

pub struct Writer<'a> {
    mem: &'a mut Memory,
    now: Timestamp,
    ops: Vec<Op>,
    staged: BTreeMap<(String, String), Entity>,
}

impl Writer<'_> {
    pub fn episode(&mut self, source: impl Into<String>, content: impl Into<String>, occurred_at: Timestamp) -> EpisodeId {
        let ep = Episode {
            id: EpisodeId::new(),
            source: source.into(),
            content: content.into(),
            occurred_at,
            created_at: self.now,
        };
        let id = ep.id;
        self.ops.push(Op::PutEpisode(ep));
        id
    }

    pub fn entity(&mut self, input: EntityInput) -> EntityId {
        let key = name_key(&input.kind, &input.name);
        let mut entity = match self.staged.get(&key) {
            Some(e) => e.clone(),
            None => match self.mem.graph.by_name.get(&key).and_then(|id| self.mem.graph.entities.get(id)) {
                Some(e) => e.clone(),
                None => Entity {
                    id: EntityId::new(),
                    name: input.name.trim().to_string(),
                    kind: input.kind.clone(),
                    summary: String::new(),
                    attributes: BTreeMap::new(),
                    embedding: None,
                    episodes: Vec::new(),
                    created_at: self.now,
                    updated_at: self.now,
                },
            },
        };
        if let Some(s) = input.summary {
            entity.summary = s;
        }
        entity.attributes.extend(input.attributes);
        if input.embedding.is_some() {
            entity.embedding = input.embedding;
        }
        if let Some(ep) = input.episode
            && !entity.episodes.contains(&ep)
        {
            entity.episodes.push(ep);
        }
        entity.updated_at = self.now;
        let id = entity.id;
        self.staged.insert(key, entity);
        id
    }

    pub fn relate(&mut self, input: RelationInput) -> Result<RelationId> {
        for id in [input.from, input.to] {
            let known = self.mem.graph.entities.contains_key(&id) || self.staged.values().any(|e| e.id == id);
            if !known {
                return Err(Error::InvalidInput(format!("unknown entity {id}")));
            }
        }
        if input.exclusive {
            let ends_at = input.valid_from.unwrap_or(self.now);
            let stale: Vec<RelationId> = self
                .mem
                .graph
                .edges(input.from, None)
                .filter(|r| r.from == input.from && r.predicate == input.predicate)
                .map(|r| r.id)
                .collect();
            for id in stale {
                self.ops.push(Op::InvalidateRelation { id, valid_to: ends_at, invalidated_at: self.now });
            }
        }
        let rel = Relation {
            id: RelationId::new(),
            from: input.from,
            to: input.to,
            predicate: input.predicate,
            fact: input.fact,
            embedding: input.embedding,
            valid_from: input.valid_from,
            valid_to: None,
            created_at: self.now,
            invalidated_at: None,
            episodes: input.episode.into_iter().collect(),
        };
        let id = rel.id;
        self.ops.push(Op::PutRelation(rel));
        Ok(id)
    }

    pub fn invalidate(&mut self, id: RelationId, valid_to: Timestamp) -> Result<()> {
        if !self.mem.graph.relations.contains_key(&id) {
            return Err(Error::InvalidInput(format!("unknown relation {id}")));
        }
        self.ops.push(Op::InvalidateRelation { id, valid_to, invalidated_at: self.now });
        Ok(())
    }

    pub async fn commit(self) -> Result<u64> {
        let mut ops: Vec<Op> = self.staged.into_values().map(Op::PutEntity).collect();
        ops.extend(self.ops);
        self.mem.commit(ops).await
    }
}
