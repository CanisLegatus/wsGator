use futures::StreamExt;
use std::pin::Pin;
use futures::stream::SplitSink;
use futures::SinkExt;
use tokio::sync::mpsc::Receiver as MpscReceiver;
use tokio::sync::watch::Receiver as WatchReceiver;
use async_trait::async_trait;
use futures::stream::SplitStream;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use crate::core::error::WsGatorError;

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

    // Starting
    fn start_writer(
        &self,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut message_rx: MpscReceiver<Message>,
    ) -> Result<(), WsGatorError> {
        tokio::spawn(async move {
            while let Some(message) = message_rx.recv().await {
                sink.send(message).await?;
            }

            Ok::<(), WsGatorError>(())
        });

        Ok(())
    }

    async fn on_connect(&self, id: u32, message_tx: &MpscSender<Message>) {
        let _ = message_tx
            .send(Message::Text(format!("Task {} started!", id).into()))
            .await;
    }
    async fn on_error(&self) {}
    async fn on_stop(&self) {
    }
    
    // This loop is to define a special logic and it is not in ordinary
    async fn get_special_loop(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }

    // Basic loop to iterate for messages and recieve stop_signal
    async fn get_basic_loop(&self, mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, message_tx: MpscSender<Message>) {
        
        loop {
            match stream.next().await {
                Some(Ok(message)) => { 
                    if let Some(message_to_send) = self.on_message(message) {
                        let _ = message_tx.send(message_to_send).await;
                    }
                },
                Some(Err(e)) => {
                    // TODO - log it!
                    println!("ERROR: {e}");
                    break;
                },
                None => {
                    // TODO - log it?
                    println!("None!");
                    break;
                },
            }
        }
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
