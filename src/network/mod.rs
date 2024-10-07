use std::sync::Arc;
use tokio::sync::mpsc;
use dashmap::DashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Send error: {0}")]
    SendError(#[from] mpsc::error::SendError<Vec<u8>>),
    #[error("Node not found")]
    NodeNotFound,
}

pub struct NetworkManager {
    nodes: Arc<DashMap<String, mpsc::Sender<Vec<u8>>>>,
}

impl NetworkManager {
    pub fn new() -> Self {
        NetworkManager {
            nodes: Arc::new(DashMap::new()),
        }
    }

    pub async fn register_node(&self, id: String, sender: mpsc::Sender<Vec<u8>>) -> Result<(), NetworkError> {
        self.nodes.insert(id, sender);
        Ok(())
    }

    pub async fn send_message(&self, to: &str, message: Vec<u8>) -> Result<(), NetworkError> {
        if let Some(sender) = self.nodes.get(to) {
            sender.send(message).await?;
            Ok(())
        } else {
            Err(NetworkError::NodeNotFound)
        }
    }
}