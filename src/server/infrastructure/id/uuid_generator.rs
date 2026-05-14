use uuid::Uuid;

use crate::server::application::ports::IdGenerator;

pub struct UuidGenerator;

impl IdGenerator for UuidGenerator {
    fn new_uuid(&self) -> Uuid {
        Uuid::new_v4()
    }
}
