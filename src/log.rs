use std::sync::Arc;

use futures::TryStreamExt;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutMode, PutOptions, PutPayload};
use serde::{Deserialize, Serialize};

use crate::model::{Op, Timestamp};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    pub seq: u64,
    pub committed_at: Timestamp,
    pub ops: Vec<Op>,
}

#[derive(Debug, Clone)]
pub struct Log {
    store: Arc<dyn ObjectStore>,
    prefix: Path,
}

impl Log {
    pub fn new(store: Arc<dyn ObjectStore>, namespace: &str) -> Self {
        let prefix = Path::from_iter(["ns", namespace, "wal"]);
        Self { store, prefix }
    }

    fn path(&self, seq: u64) -> Path {
        // zero-padded so listing order is commit order
        self.prefix.clone().join(format!("{seq:020}.json"))
    }

    pub async fn append(&self, batch: &Batch) -> Result<()> {
        let body = serde_json::to_vec(batch)?;
        // create-only put, so if another writer took this seq first we get a conflict
        let opts = PutOptions {
            mode: PutMode::Create,
            ..Default::default()
        };
        match self
            .store
            .put_opts(&self.path(batch.seq), PutPayload::from(body), opts)
            .await
        {
            Ok(_) => Ok(()),
            Err(object_store::Error::AlreadyExists { .. }) => Err(Error::Conflict { seq: batch.seq }),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn read_from(&self, from: u64) -> Result<Vec<Batch>> {
        let mut paths: Vec<Path> = self
            .store
            .list(Some(&self.prefix))
            .map_ok(|meta| meta.location)
            .try_collect()
            .await?;
        paths.sort();

        let mut batches = Vec::new();
        let mut expected = from;
        for path in paths {
            let Some(seq) = path
                .filename()
                .and_then(|f| f.strip_suffix(".json"))
                .and_then(|s| s.parse::<u64>().ok())
            else {
                continue;
            };
            if seq < from {
                continue;
            }
            if seq != expected {
                return Err(Error::Corrupt(format!("log gap: expected seq {expected}, found {seq}")));
            }
            let bytes = self.store.get(&path).await?.bytes().await?;
            let batch: Batch = serde_json::from_slice(&bytes)?;
            if batch.seq != seq {
                return Err(Error::Corrupt(format!("object {path} holds seq {}", batch.seq)));
            }
            batches.push(batch);
            expected += 1;
        }
        Ok(batches)
    }
}
