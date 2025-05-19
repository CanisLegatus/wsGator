use crate::CommonConfig;
use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::watch::Receiver;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use futures::StreamExt;
use futures::SinkExt;

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
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>>;
    
    async fn handle_base_events(&self, mut ws: WebSocketStream<MaybeTlsStream<TcpStream>>, i: u32) -> Result<bool, WsError> {
                        

        if let Some(msg) = ws.next().await {
            let (proceed, message) = self.handle_messages(msg, i);

            if let Some(message) = message {
                ws.send(message).await?;
            }
            
            if !proceed {
                return Ok(false);
            } else {
                return Ok(true);
            }
        } else {
            return Ok(true);
        }
    }

    fn handle_messages(
        &self,
        msg: Result<Message, tokio_tungstenite::tungstenite::Error>,
        connection_number: u32,
    ) -> (bool, Option<Message>) {
        match msg {
            Ok(message) => match message {
                Message::Ping(bytes) => (true, Some(Message::Pong(bytes))),

                Message::Text(txt) => {
                    println!("Connection {}: Recieved a text: {}", connection_number, txt);
                    (true, None)
                }
                Message::Close(_) => (false, None),

                _ => (false, None),
            },
            Err(e) => {
                println!(
                    "Connection {}: Recieved Err inside a message on listening to next message: {}",
                    connection_number, e
                );
                (false, None)
            }
        }
    }
}
