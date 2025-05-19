use tokio::sync::watch::Receiver;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use std::pin::Pin;
use std::sync::Arc;
use tokio_tungstenite::MaybeTlsStream;
use tokio::net::TcpStream;
use async_trait::async_trait;
use crate::CommonConfig;

use tokio_tungstenite::tungstenite::Error as WsError;

// Enumiration for TypeChecking while getting user input from CLI
#[derive(clap::ValueEnum, Clone, Copy)]
pub enum AttackStrategyType {
    Flat,
    RampUp,
    Flood,
}

#[async_trait]
pub trait AttackStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig>;
    fn run_connection_loop(
        self: Arc<Self>,
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        rx: Option<Receiver<bool>>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + Sync + 'static>>;


    fn handle_messages(
        &self,
        msg: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
        connection_number: u32,
    ) -> (bool, Option<Message>) {
        match msg {
            Some(Ok(message)) => match message {
                Message::Ping(bytes) => (true, Some(Message::Pong(bytes))),

                Message::Text(txt) => {
                    println!("Connection {}: Recieved a text: {}", connection_number, txt);
                    (true, None)
                }
                Message::Close(_) => (false, None),

                _ => (false, None),
            },
            Some(Err(e)) => {
                println!(
                    "Connection {}: Recieved Err inside a message on listening to next message: {}",
                    connection_number, e
                );
                (false, None)
            }
            None => {
                println!(
                    "Connection {}: Recieved None on listening to next message",
                    connection_number
                );
                (false, None)
            }
        }
    }
}
