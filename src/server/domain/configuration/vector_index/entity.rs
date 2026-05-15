use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::VectorStoreKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorIndex {
    pub index_id: Uuid,
    pub kind: VectorStoreKind,
    pub name: String,
    pub dimensions: u32,
}
