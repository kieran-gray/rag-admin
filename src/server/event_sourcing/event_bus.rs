use std::sync::Arc;

use tokio::sync::broadcast;

use super::envelope::PublishedEvent;

const BUS_CAPACITY: usize = 1024;

pub struct EventBus {
    sender: broadcast::Sender<Arc<PublishedEvent>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self { sender }
    }

    pub fn publish(&self, event: Arc<PublishedEvent>) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> EventBusSubscription {
        EventBusSubscription {
            receiver: self.sender.subscribe(),
        }
    }
}

pub struct EventBusSubscription {
    receiver: broadcast::Receiver<Arc<PublishedEvent>>,
}

impl EventBusSubscription {
    pub async fn recv(&mut self) -> Result<Arc<PublishedEvent>, broadcast::error::RecvError> {
        self.receiver.recv().await
    }
}
