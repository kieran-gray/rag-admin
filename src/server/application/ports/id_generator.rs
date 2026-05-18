use uuid::Uuid;

pub trait IdGenerator: Send + Sync {
    fn new_uuid(&self) -> Uuid;
}
