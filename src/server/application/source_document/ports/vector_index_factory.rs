use std::sync::Arc;

use crate::server::application::indexing::ports::VectorIndex;

pub trait VectorIndexProvider: Send + Sync {
    fn build(&self, index_name: &str, dimensions: u32) -> Arc<dyn VectorIndex>;
}
