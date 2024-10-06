mod transport;

use self::transport::Transport;
use std::sync::Arc;
use tokio::sync::mpsc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("Send error: {0}")]
    SendError(#[from] mpsc::error::SendError<Vec<u8>>),
    // Define other network-related errors
}

pub struct NetworkManager {
    transport: Arc<dyn Transport>,
}

impl NetworkManager {
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        NetworkManager { transport }
    }

    pub async fn register_node(&self, id: String, sender: mpsc::Sender<Vec<u8>>) -> Result<(), NetworkError> {
        self.transport.register_node(id, sender).await
    }

    pub async fn send_message(&self, to: &str, message: Vec<u8>) -> Result<(), NetworkError> {
        self.transport.send_message(to, message).await
    }
}
