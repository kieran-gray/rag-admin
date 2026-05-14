use serde::{de::DeserializeOwned, Serialize};

pub trait Aggregate: Sized + Clone + Serialize + DeserializeOwned + Send + Sync {
    type Event: Clone + Serialize + DeserializeOwned + Send + Sync + 'static;
    type Command: Send + Sync + 'static;
    type Error: std::error::Error + Send + Sync + 'static;

    fn aggregate_type() -> &'static str;

    fn apply(&mut self, event: &Self::Event);

    fn handle_command(
        state: Option<&Self>,
        command: Self::Command,
    ) -> Result<Vec<Self::Event>, Self::Error>;

    fn from_events(events: &[Self::Event]) -> Option<Self>;
}
