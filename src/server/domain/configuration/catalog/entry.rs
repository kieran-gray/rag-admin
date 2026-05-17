use uuid::Uuid;

use super::error::CatalogError;

pub trait CatalogEntry: Clone {
    fn id(&self) -> Uuid;

    fn natural_key(&self) -> String;

    fn validate(&self) -> Result<(), CatalogError>;
}
