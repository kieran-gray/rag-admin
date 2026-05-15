use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::catalog::VectorStoreKind;
use crate::contracts::VectorIndexCommandDto;

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

impl VectorIndexCatalogCommand {
    pub fn from_dto(dto: VectorIndexCommandDto) -> Self {
        match dto {
            VectorIndexCommandDto::AddVectorIndex(d) => Self::AddVectorIndex(AddVectorIndex {
                kind: d.kind,
                name: d.name,
                dimensions: d.dimensions,
            }),
            VectorIndexCommandDto::UpdateVectorIndex(d) => {
                Self::UpdateVectorIndex(UpdateVectorIndex {
                    index_id: d.index_id,
                    kind: d.kind,
                    name: d.name,
                    dimensions: d.dimensions,
                })
            }
            VectorIndexCommandDto::RemoveVectorIndex(d) => {
                Self::RemoveVectorIndex(RemoveVectorIndex {
                    index_id: d.index_id,
                })
            }
        }
    }
}
