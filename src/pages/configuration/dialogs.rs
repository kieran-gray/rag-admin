use crate::shared::{
    EmbeddingModelDto, GenerationModelDto, PipelineConfigurationDto, VectorIndexDto,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddDialog {
    EmbeddingModel,
    GenerationModel,
    VectorIndex,
    PipelineConfiguration,
}

#[derive(Debug, Clone)]
pub enum EditDialog {
    EmbeddingModel(EmbeddingModelDto),
    GenerationModel(GenerationModelDto),
    VectorIndex(VectorIndexDto),
    PipelineConfiguration(PipelineConfigurationDto),
}

#[derive(Debug, Clone)]
pub enum DeleteDialog {
    EmbeddingModel(EmbeddingModelDto),
    GenerationModel(GenerationModelDto),
    VectorIndex(VectorIndexDto),
}

pub fn delete_dialog_label(dialog: Option<DeleteDialog>) -> String {
    match dialog {
        Some(DeleteDialog::EmbeddingModel(m)) => format!("Delete embedding model '{}'.", m.model),
        Some(DeleteDialog::GenerationModel(m)) => {
            format!("Delete generation model '{}'.", m.model)
        }
        Some(DeleteDialog::VectorIndex(i)) => format!("Delete vector index '{}'.", i.name),
        None => String::new(),
    }
}
