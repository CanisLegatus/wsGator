use crate::CommonConfig;
use async_trait::async_trait;
use futures::SinkExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use futures::StreamExt;
use futures::stream::SplitSink;
use tokio::sync::mpsc::{Sender as MpscSender, Receiver as MpscReceiver};
use tokio::sync::watch::Receiver as WatchReceiver;
use futures::stream::SplitStream;
use tokio_tungstenite::tungstenite::Error as WsError;

// Enumiration for TypeChecking while getting user input from CLI
#[derive(clap::ValueEnum, Clone, Copy)]
pub enum AttackStrategyType {
    Flat,
    RampUp,
    Flood,
}

#[async_trait]
pub trait AttackStrategy: Send + Sync {
    fn get_common_config(&self) -> Arc<CommonConfig>;
    fn run_connection_loop(
        self: Arc<Self>,
        stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        stop_rx: Option<WatchReceiver<bool>>, 
        writer_tx: MpscSender<Message>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>>;
    
    async fn run_writer(&self, mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>, mut writer_rx: MpscReceiver<Message>) -> Result<(), WsError> {
        
        tokio::spawn(
            async move {
                loop {
                    tokio::select! {
                        opt_message = writer_rx.recv() => {
                            sink.send(message).await;
                        }
                    }
                }
            }
        );

        Ok(())
    }

    async fn handle_base_events(&self, mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>, writer_tx: MpscSender<Message>, i: u32) -> Result<bool, WsError> {
        if let Some(msg) = stream.next().await {
            let (proceed, message) = self.handle_messages(msg, i);

            if let Some(message) = message {
                writer_tx.send(message);
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
