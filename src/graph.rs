use std::collections::HashMap;

use crate::model::{Entity, EntityId, Episode, EpisodeId, Op, Relation, RelationId};

#[derive(Debug, Default, Clone)]
pub struct Graph {
    pub(crate) entities: HashMap<EntityId, Entity>,
    pub(crate) relations: HashMap<RelationId, Relation>,
    pub(crate) episodes: HashMap<EpisodeId, Episode>,
    pub(crate) adjacency: HashMap<EntityId, Vec<RelationId>>,
    pub(crate) by_name: HashMap<(String, String), EntityId>,
}

pub(crate) fn name_key(kind: &str, name: &str) -> (String, String) {
    (kind.to_string(), name.trim().to_lowercase())
}

impl Graph {
    pub fn apply(&mut self, op: &Op) {
        match op {
            Op::PutEpisode(ep) => {
                self.episodes.insert(ep.id, ep.clone());
            }
            Op::PutEntity(entity) => {
                if let Some(old) = self.entities.get(&entity.id) {
                    self.by_name.remove(&name_key(&old.kind, &old.name));
                }
                self.by_name.insert(name_key(&entity.kind, &entity.name), entity.id);
                self.entities.insert(entity.id, entity.clone());
            }
            Op::PutRelation(rel) => {
                if !self.relations.contains_key(&rel.id) {
                    self.adjacency.entry(rel.from).or_default().push(rel.id);
                    if rel.to != rel.from {
                        self.adjacency.entry(rel.to).or_default().push(rel.id);
                    }
                }
                self.relations.insert(rel.id, rel.clone());
            }
            Op::InvalidateRelation { id, valid_to, invalidated_at } => {
                if let Some(rel) = self.relations.get_mut(id) {
                    rel.valid_to = Some(rel.valid_to.map_or(*valid_to, |v| v.min(*valid_to)));
                    rel.invalidated_at.get_or_insert(*invalidated_at);
                }
            }
        }
    }

    pub fn edges(&self, id: EntityId, as_of: Option<i64>) -> impl Iterator<Item = &Relation> {
        self.adjacency
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|rid| self.relations.get(rid))
            .filter(move |r| r.is_active(as_of))
    }
}
