use crate::server::domain::shared::Timestamp;

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

pub struct FixedClock(pub Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0.clone()
    }
}
