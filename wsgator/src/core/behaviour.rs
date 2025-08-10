use std::time::Duration;

use async_trait::async_trait;
use futures::stream::SplitStream;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

#[async_trait]
pub trait Behaviour: Send + Sync {
    async fn on_message(&self) {}
    async fn on_connect(&self, id: u32, message_tx: &MpscSender<Message>) {
        let _ = message_tx
            .send(Message::Text(format!("Task {} started!", id).into()))
            .await;
    }
    async fn on_error(&self) {}
    async fn on_start(&self) {}
    async fn on_stop(&self) {}

    // Main function of behaviour
    // TODO!
    // Implement basic behaviour (modules?)
    async fn run(
        &self,
        id: u32,
        stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        message_tx: MpscSender<Message>,
    ) {
        self.on_connect(id, &message_tx).await;

        tokio::time::sleep(Duration::from_secs(10)).await;

        self.on_stop().await;
    }
}

// Structs

pub struct SilentBehaviour {}

pub struct PingPongBehaviour {}

pub struct FloodBehaviour {}

// Implementations

#[async_trait]
impl Behaviour for SilentBehaviour {}

#[async_trait]
impl Behaviour for PingPongBehaviour {}

#[async_trait]
impl Behaviour for FloodBehaviour {}
