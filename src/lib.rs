pub mod graph;
pub mod log;
pub mod memory;
pub mod model;

pub use memory::{EntityInput, Memory, Recall, RecallResult, RelationInput, Scored, Writer};
pub use model::{Entity, EntityId, Episode, EpisodeId, Op, Relation, RelationId, Timestamp};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("write conflict at seq {seq}: another writer committed first")]
    Conflict { seq: u64 },
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("corrupt log: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Store(#[from] object_store::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
