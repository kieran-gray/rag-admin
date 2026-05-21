use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmbeddingModelRef {
    pub embedding_model_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VectorIndexRef {
    pub vector_index_id: Uuid,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddIndexProfile {
    pub name: String,
    pub embedding_model: EmbeddingModelRef,
    pub vector_index: VectorIndexRef,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateIndexProfile {
    pub index_profile_id: Uuid,
    pub name: String,
    pub embedding_model: EmbeddingModelRef,
    pub vector_index: VectorIndexRef,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveIndexProfile {
    pub index_profile_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum IndexProfileCatalogCommand {
    AddIndexProfile(AddIndexProfile),
    UpdateIndexProfile(UpdateIndexProfile),
    RemoveIndexProfile(RemoveIndexProfile),
}
