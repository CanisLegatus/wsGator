use futures::StreamExt;
use std::pin::Pin;
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
    
    // This loop is to define a special logic and it is not in ordinary
    fn special_loop(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }

    // Basic loop to iterate for messages and recieve stop_signal
    async fn basic_loop(&self, mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, message_tx: MpscSender<Message>) {
        
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
        
        // Special loop executes as a different thread
        if let Some(task) = self.special_loop() {
            tokio::spawn(task);
        }

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
