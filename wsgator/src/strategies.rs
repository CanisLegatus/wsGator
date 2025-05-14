use crate::Args;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
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
        rx: Option<Receiver<bool>>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>>;
    async fn run(self: Arc<Self>) {
        let config = self.get_common_config();

        for _wave in 0..config.waves_number {
            let con = Arc::clone(&config);
            // Creating independent watch_channel to stop all tasks extenally
            let mut watch_channel: Option<Receiver<bool>> = None;
            if config.external_timer {
                let (stop_tx, stop_rx) = watch::channel(false);
                watch_channel = Some(stop_rx);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(con.connection_duration)).await;
                    let _ = stop_tx.send(true);
                });
            };

            let mut tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>>> = vec![];

            // Creating connections
            for i in 0..config.connection_number {
                let strategy = Arc::clone(&self);
                let con = Arc::clone(&config);
                let rx = watch_channel.clone();

                // Spawning thread for each connection
                tasks.push(strategy.run_connection_loop(rx, con, i));

                // TODO! Warning - this feature was cropped! I need to reimplement
                // Pause between connections if applied
                tokio::time::sleep(Duration::from_millis(config.connection_pause)).await;
            }

            // Waves logic
            let _: Vec<_> = tasks.into_iter().map(tokio::spawn).collect();

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
        rx: Option<Receiver<bool>>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>> {
        let url = config.url_under_fire.clone();
        let spam_pause = self.spam_pause;

        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return,
            };

            match connect_async(&url).await {
                Ok((mut ws, _)) => {
                    if let Err(e) = ws
                        .send(Message::Text(format!("Peer {} saying Hello!", i).into()))
                        .await
                    {
                        println!("Connection {}: Can't send HELLO on connection: {}", i, e);
                        return;
                    }
                    // This loop can be in .await func
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
        rx: Option<Receiver<bool>>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>> {
        let url = config.url_under_fire.clone();

        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return,
            };

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
        rx: Option<Receiver<bool>>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + Sync + 'static>> {
        let url = config.url_under_fire.clone();
        //let mut interval = interval(Duration::from_secs(config.connection_duration));
        //interval.tick().await;

        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return,
            };

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
        })
    }
}
