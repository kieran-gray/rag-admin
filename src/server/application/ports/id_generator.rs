use uuid::Uuid;

pub trait IdGenerator: Send + Sync {
    fn new_uuid(&self) -> Uuid;
}

pub struct FixedIdGenerator(pub Uuid);

impl IdGenerator for FixedIdGenerator {
    fn new_uuid(&self) -> Uuid {
        self.0
    }
}
