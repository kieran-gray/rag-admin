use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::reference_data::VectorStoreKind;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AddVectorIndex {
    pub kind: VectorStoreKind,
    pub name: String,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UpdateVectorIndex {
    pub index_id: Uuid,
    pub kind: VectorStoreKind,
    pub name: String,
    pub dimensions: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoveVectorIndex {
    pub index_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VectorIndexCatalogCommand {
    AddVectorIndex(AddVectorIndex),
    UpdateVectorIndex(UpdateVectorIndex),
    RemoveVectorIndex(RemoveVectorIndex),
}
