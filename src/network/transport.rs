use async_trait::async_trait;
use tokio::sync::mpsc;
use std::sync::Arc;
use dashmap::DashMap;

#[async_trait]
pub trait Transport: Send + Sync {
    async fn register_node(&self, id: String, sender: mpsc::Sender<Vec<u8>>) -> Result<(), super::NetworkError>;
    async fn send_message(&self, to: &str, message: Vec<u8>) -> Result<(), super::NetworkError>;
}

pub struct InMemoryTransport {
    nodes: Arc<DashMap<String, mpsc::Sender<Vec<u8>>>>,
}

impl InMemoryTransport {
    pub fn new() -> Self {
        InMemoryTransport {
            nodes: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl Transport for InMemoryTransport {
    async fn register_node(&self, id: String, sender: mpsc::Sender<Vec<u8>>) -> Result<(), super::NetworkError> {
        self.nodes.insert(id, sender);
        Ok(())
    }

    async fn send_message(&self, to: &str, message: Vec<u8>) -> Result<(), super::NetworkError> {
        if let Some(sender) = self.nodes.get(to) {
            sender.send(message).await?;
        }
        Ok(())
    }
}
