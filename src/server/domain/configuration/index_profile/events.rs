use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IndexProfileCatalogCreated {
    pub catalog_id: Uuid,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IndexProfileAdded {
    pub index_profile_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IndexProfileUpdated {
    pub index_profile_id: Uuid,
    pub name: String,
    pub embedding_model_id: Uuid,
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct IndexProfileRemoved {
    pub index_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum IndexProfileCatalogEvent {
    IndexProfileCatalogCreated(IndexProfileCatalogCreated),
    IndexProfileAdded(IndexProfileAdded),
    IndexProfileUpdated(IndexProfileUpdated),
    IndexProfileRemoved(IndexProfileRemoved),
}
