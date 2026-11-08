use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// unix millis
pub type Timestamp = i64;

pub fn now() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as Timestamp)
        .unwrap_or(0)
}

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_type!(EntityId);
id_type!(RelationId);
id_type!(EpisodeId);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub id: EpisodeId,
    pub source: String,
    pub content: String,
    pub occurred_at: Timestamp,
    pub created_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub kind: String,
    pub summary: String,
    pub attributes: BTreeMap<String, serde_json::Value>,
    pub embedding: Option<Vec<f32>>,
    pub episodes: Vec<EpisodeId>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub id: RelationId,
    pub from: EntityId,
    pub to: EntityId,
    pub predicate: String,
    pub fact: String,
    pub embedding: Option<Vec<f32>>,
    // valid time: when the fact held in the world
    pub valid_from: Option<Timestamp>,
    pub valid_to: Option<Timestamp>,
    // transaction time: when it was recorded and when it was closed
    pub created_at: Timestamp,
    pub invalidated_at: Option<Timestamp>,
    pub episodes: Vec<EpisodeId>,
}

impl Relation {
    pub fn is_active(&self, as_of: Option<Timestamp>) -> bool {
        match as_of {
            // none means the current state
            None => self.invalidated_at.is_none() && self.valid_to.is_none(),
            Some(t) => {
                self.valid_from.is_none_or(|from| from <= t)
                    && self.valid_to.is_none_or(|to| t < to)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    PutEpisode(Episode),
    PutEntity(Entity),
    PutRelation(Relation),
    InvalidateRelation {
        id: RelationId,
        valid_to: Timestamp,
        invalidated_at: Timestamp,
    },
}
