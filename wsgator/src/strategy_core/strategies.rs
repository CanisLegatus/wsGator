use crate::Args;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::watch::Receiver;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::{connect_async, tungstenite::Message};

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
    async fn get_ws_connection(
        &self,
        url: &str,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let (ws, _) = connect_async(url).await?;
        Ok(ws)
    }

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

#[derive(Clone)]
pub struct CommonConfig {
    pub url_under_fire: String,
    pub connection_number: u32,
    pub connection_duration: u64,
    pub connection_pause: u64,
    pub waves_number: u32,
    pub waves_pause: u64,
    pub external_timer: bool,
}

impl From<Args> for CommonConfig {
    fn from(value: Args) -> Self {
        Self {
            url_under_fire: value.url.clone(),
            connection_number: value.connection_number,
            connection_duration: value.connection_duration,
            connection_pause: value.connection_pause,
            waves_number: value.waves_number,
            waves_pause: value.waves_pause,
            external_timer: false,
        }
    }
}

impl CommonConfig {
    pub fn with_external_timer(mut self) -> Self {
        self.external_timer = true;
        self
    }
}

pub struct FlatStrategy {
    pub common_config: Arc<CommonConfig>,
}
pub struct RampUpStrategy {
    pub common_config: Arc<CommonConfig>,
}
pub struct FloodStrategy {
    pub common_config: Arc<CommonConfig>,
    pub spam_pause: u64,
}

#[async_trait]
impl AttackStrategy for FloodStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }
    fn run_connection_loop(
        self: Arc<Self>,
        mut ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        rx: Option<Receiver<bool>>,
        _config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + Sync + 'static>> {
        let spam_pause = self.spam_pause;

        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return Ok(()),
            };
            loop {
                tokio::select! {
                    msg = ws.next() => {
                        let (proceed, message) = self.handle_messages(msg, i);

                        if let Some(message) = message {
                            ws.send(message).await?;
                        }

                        if !proceed {
                            break Ok(());
                        }
                    },

                    // Spamming with constant messages
                    _ = tokio::time::sleep(Duration::from_millis(spam_pause)) => {
                       ws.send(Message::Text(format!("SPAM! From connection {}", i).into())).await?;
                    }

                    _ = rx.changed() => {
                        println!("Connection {}: Reached its target time. Dropping connection", i);
                        ws.send(Message::Close(None)).await?;
                        break Ok(());
                    }
                }
            }
        })
    }
}

#[async_trait]
impl AttackStrategy for FlatStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }
    fn run_connection_loop(
        self: Arc<Self>,
        mut ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        rx: Option<Receiver<bool>>,
        _config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + Sync + 'static>> {
        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return Ok(()),
            };
            // Connection loop
            loop {
                tokio::select! {
                    msg = ws.next() => {
                        let (proceed, message) = self.handle_messages(msg, i);

                        if let Some(message) = message {
                            ws.send(message).await?;
                        }

                        if !proceed {
                            break Ok(());
                        }
                    },

                    _ = rx.changed() => {
                        println!("Connection {}: Reached its target time. Sending Close Frame", i);
                        ws.send(Message::Close(None)).await?;
                        break Ok(());
                    }
                }
            }
        })
    }
}

#[async_trait]
impl AttackStrategy for RampUpStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }
    fn run_connection_loop(
        self: Arc<Self>,
        mut ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        rx: Option<Receiver<bool>>,
        _config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + Sync + 'static>> {
        //let mut interval = interval(Duration::from_secs(config.connection_duration));
        //interval.tick().await;

        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return Ok(()),
            };
            // Connection loop
            loop {
                tokio::select! {
                    msg = ws.next() => {
                        let (proceed, message) = self.handle_messages(msg, i);

                        if let Some(message) = message {
                            ws.send(message).await?;
                        }

                        if !proceed {
                            break Ok(());
                        }
                    },

                    _ = rx.changed() => {
                        println!("Connection {}: Reached its target time. Sending Close Frame", i);
                        ws.send(Message::Close(None)).await?;
                        break Ok(());
                    }
                }
            }
        })
    }
}
