use crate::server::domain::shared::Timestamp;

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}
