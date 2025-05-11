use crate::Args;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};

// Enumiration for TypeChecking while getting user input from CLI
#[derive(clap::ValueEnum, Clone, Copy)]
pub enum AttackStrategyType {
    Flat,
    RampUp,
    Flood,
}

pub struct AttackConfig {
    url_under_fire: String,
    connection_number: u32,
    connection_duration: u64,
    connection_pause: Option<u64>,
    waves_number: u32,
    waves_pause: u64,
    spam_pause: Option<u64>,
    external_timer: Option<u64>,
}

impl From<Args> for AttackConfig {
    fn from(value: Args) -> Self {
        let (spam_pause, external_timer, connection_pause) = match value.strategy {
            AttackStrategyType::Flat => (None, Some(value.connection_duration), None),
            AttackStrategyType::RampUp => (None, None, Some(value.connection_pause)),
            AttackStrategyType::Flood => (
                Some(value.connection_pause),
                Some(value.connection_duration),
                None,
            ),
        };

        AttackConfig {
            url_under_fire: value.url,
            connection_number: value.connection_number,
            connection_duration: value.connection_duration,
            connection_pause,
            waves_number: value.waves_number,
            waves_pause: value.waves_pause,
            spam_pause,
            external_timer,
        }
    }
}

#[async_trait]
pub trait AttackStrategy {
    async fn run_connection_loop(
        self: Arc<Self>,
        config: &AttackConfig,
        stop_rx: Option<Receiver<bool>>,
        i: u32,
    );
    async fn run(self: Arc<Self>, config: &AttackConfig) {
        for _wave in 0..config.waves_number {
            // Creating independent watch_channel to stop all tasks extenally
            let mut watch_channel: Option<Receiver<bool>> = None;
            if let Some(timer) = config.external_timer {
                let (stop_tx, stop_rx) = watch::channel(false);
                watch_channel = Some(stop_rx);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(timer)).await;
                    let _ = stop_tx.send(true);
                });
            };

            // Creating connections
            for i in 0..config.connection_number {
                let strategy = Arc::clone(&self);
                let rx = watch_channel.clone();

                // Spawning thread for each connection
                strategy.run_connection_loop(config, rx, i).await;

                // Pause between connections if applied
                if let Some(connection_pause) = config.connection_pause {
                    tokio::time::sleep(Duration::from_millis(connection_pause)).await;
                }
            }

            // Waves delay timer
            tokio::time::sleep(Duration::from_secs(config.waves_pause)).await;
        }
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

pub struct FlatStrategy;
pub struct RampUpStrategy;
pub struct FloodStrategy;

#[async_trait]
impl AttackStrategy for FloodStrategy {
    async fn run_connection_loop(
        self: Arc<Self>,
        config: &AttackConfig,
        rx: Option<Receiver<bool>>,
        i: u32,
    ) {
        let url = config.url_under_fire.clone();
        let spam_pause = config.spam_pause.unwrap_or(100);

        let mut rx = if let Some(rx) = rx {
            rx
        } else {
            return;
        };

        tokio::spawn(async move {
            match connect_async(&url).await {
                Ok((mut ws, _)) => {
                    if let Err(e) = ws
                        .send(Message::Text(format!("Peer {} saying Hello!", i).into()))
                        .await
                    {
                        println!("Connection {}: Can't send HELLO on connection: {}", i, e);
                        return;
                    }

                    // Main strategy logic loop
                    loop {
                        tokio::select! {
                            msg = ws.next() => {
                                let (proceed, message) = self.handle_messages(msg, i);

                                if let Some(message) = message {
                                    if let Err(e) = ws.send(message).await {
                                        println!("Connection: {}: Can't send message: {}", i, e);
                                        break;
                                    }
                                }

                                if !proceed {
                                    break;
                                }
                            },

                            // Spamming with constant messages
                            _ = tokio::time::sleep(Duration::from_millis(spam_pause)) => {
                               if let Err(e) = ws.send(Message::Text(format!("SPAM! From connection {}", i).into())).await {
                                   println!("Connection {}: Can't send SPAM message! Error: {}", i, e);
                                   break;
                               }
                            }

                            _ = rx.changed() => {
                                // Doing "dirty" connection drop
                                println!("Connection {}: Reached its target time. Dropping connection", i);
                                drop(ws);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Connection {}: Error on WS connection: {}", i, e);
                }
            }
        });
    }
}

#[async_trait]
impl AttackStrategy for FlatStrategy {
    async fn run_connection_loop(
        self: Arc<Self>,
        config: &AttackConfig,
        rx: Option<Receiver<bool>>,
        i: u32,
    ) {
        let url = config.url_under_fire.clone();

        let mut rx = if let Some(rx) = rx {
            rx
        } else {
            return;
        };

        tokio::spawn(async move {
            match connect_async(&url).await {
                Ok((mut ws, _)) => {
                    if let Err(e) = ws
                        .send(Message::Text(format!("Peer {} saying Hello!", i).into()))
                        .await
                    {
                        println!("Connection {}: Can't send HELLO on connection: {}", i, e);
                        return;
                    }

                    // Connection loop
                    loop {
                        tokio::select! {
                            msg = ws.next() => {
                                let (proceed, message) = self.handle_messages(msg, i);

                                if let Some(message) = message {
                                    if let Err(e) = ws.send(message).await {
                                        println!("Connection: {}: Can't send message: {}", i, e);
                                        break;
                                    }
                                }

                                if !proceed {
                                    break;
                                }
                            },

                            _ = rx.changed() => {
                                // Doing "dirty" connection drop
                                println!("Connection {}: Reached its target time. Sending Close Frame", i);
                                drop(ws);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Connection {}: Error on WS connection: {}", i, e);
                }
            }
        });
    }
}

#[async_trait]
impl AttackStrategy for RampUpStrategy {
    async fn run_connection_loop(
        self: Arc<Self>,
        config: &AttackConfig,
        _rx: Option<Receiver<bool>>,
        i: u32,
    ) {
        let url = config.url_under_fire.clone();
        let mut interval = interval(Duration::from_secs(config.connection_duration));
        interval.tick().await;

        tokio::spawn(async move {
            match connect_async(&url).await {
                Ok((mut ws, _)) => {
                    if let Err(e) = ws
                        .send(Message::Text(format!("Peer {} saying Hello!", i).into()))
                        .await
                    {
                        println!("Connection {}: Can't send HELLO on connection: {}", i, e);
                        return;
                    }

                    // Connection loop
                    loop {
                        tokio::select! {
                            msg = ws.next() => {
                                let (proceed, message) = self.handle_messages(msg, i);

                                if let Some(message) = message {
                                    if let Err(e) = ws.send(message).await {
                                        println!("Connection: {}: Can't send message: {}", i, e);
                                        break;
                                    }
                                }

                                if !proceed {
                                    break;
                                }
                            },

                            _ = interval.tick() => {
                                println!("Connection {}: Reached its target time. Sending Close Frame", i);

                                if let Err(e) = ws.send(Message::Close(None)).await {
                                    println!("Connection: {}: Can't send Close Frame: {}", i, e);
                                }
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Connection {}: Error on WS connection: {}", i, e);
                }
            }
        });
    }
}
