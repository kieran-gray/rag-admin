pub mod chunking_configuration;
pub mod embedding_model;
pub mod generation_model;
pub mod pipeline_configuration;
pub mod sweep_template;
pub mod vector_index;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use uuid::Uuid;

    use super::{
        embedding_model::EmbeddingModelCatalog, generation_model::GenerationModelCatalog,
        vector_index::VectorIndexCatalog,
    };

    #[test]
    fn singleton_catalog_stream_ids_are_global_and_distinct() {
        let ids = [
            EmbeddingModelCatalog::singleton_id(),
            GenerationModelCatalog::singleton_id(),
            VectorIndexCatalog::singleton_id(),
        ];

        assert!(ids.iter().all(|id| *id != Uuid::nil()));
        assert_eq!(ids.into_iter().collect::<HashSet<_>>().len(), ids.len());
    }
}
