use futures::StreamExt;
use tokio::sync::watch::Receiver as WatchReceiver;
use async_trait::async_trait;
use futures::stream::SplitStream;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

#[async_trait]
pub trait Behaviour: Send + Sync {
    fn on_message(&self, message: Message) -> Option<Message> {
        match message {
            Message::Ping(bytes) => {
                Some(Message::Pong(bytes))
            },
            Message::Text(text) => {
                Some(Message::Text(format!("Server sent me text! {text}").into()))
            }
            _ => {
                None
            }
        }
    }
    async fn on_connect(&self, id: u32, message_tx: &MpscSender<Message>) {
        let _ = message_tx
            .send(Message::Text(format!("Task {} started!", id).into()))
            .await;
    }
    async fn on_error(&self) {}
    async fn on_stop(&self) {
    }
    async fn basic_loop(&self, mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, message_tx: MpscSender<Message>) {
        
        loop {
            match stream.next().await {
                Some(Ok(message)) => { 
                    match self.on_message(message) {
                        Some(message_to_send) => {
                            let _ = message_tx.send(message_to_send).await;
                        },
                        None => {
                            // TODO
                            // How can we handle and react 
                        }
                    }
                },
                Some(Err(e)) => {
                    // TODO - log it!
                    break;
                },
                None => {
                    // TODO - log it?
                    break;
                },
            }
        }
    }

    // Main function of behaviour
    // TODO!
    // Implement basic behaviour (modules?)
    async fn run(
        &self,
        id: u32,
        stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        message_tx: MpscSender<Message>,
        mut stop_rx: WatchReceiver<bool>,
    ) {
        self.on_connect(id, &message_tx).await;
        
        // Here we have a basic loop
        tokio::select! {
            _ = self.basic_loop(stream, message_tx.clone()) => {},
            _ = stop_rx.changed() => {
                println!("Death on timer!");
            },
        }

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
