use crate::Args;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::interval;
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
    fn name(&self) -> AttackStrategyType;
    async fn run(self: Arc<Self>, args: &Args);
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
    fn name(&self) -> AttackStrategyType {
        AttackStrategyType::Flood
    }

    async fn run(self: Arc<Self>, args: &Args) {
        for _wave in 0..args.waves_amount {
            let (tx, rx) = watch::channel(false);
            let mut interval = interval(Duration::from_secs(args.connection_duration as u64));
            interval.tick().await;

            // Creating independent timer
            let timer = args.connection_duration;

            //Spawning timer to cancell all threads
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(timer as u64)).await;
                let _ = tx.send(true);
            });

            // Creating connections
            for i in 0..args.connections {
                let url = args.url.clone();
                let strat = Arc::clone(&self);
                let mut rx = rx.clone();
                let spam_pause = args.spam_pause;

                // Spawning thread for each connection
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
                                        let (proceed, message) = strat.handle_messages(msg, i);

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
                                    _ = tokio::time::sleep(Duration::from_millis(spam_pause as u64)) => {
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
    }
}

#[async_trait]
impl AttackStrategy for FlatStrategy {
    fn name(&self) -> AttackStrategyType {
        AttackStrategyType::Flat
    }
    async fn run(self: Arc<Self>, args: &Args) {
        for _wave in 0..args.waves_amount {
            let (tx, rx) = watch::channel(false);
            let mut interval = interval(Duration::from_secs(args.connection_duration as u64));
            interval.tick().await;

            // Creating independent timer
            let timer = args.connection_duration;

            // Spawning timer to cancell all threads
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(timer as u64)).await;
                let _ = tx.send(true);
            });

            // Creating connetctions
            for i in 0..args.connections {
                let url = args.url.clone();
                let strat = Arc::clone(&self);
                let mut rx = rx.clone();

                // Spawning threads for each connection
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
                                        let (proceed, message) = strat.handle_messages(msg, i);

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
                // New connection timer
                tokio::time::sleep(Duration::from_millis(0)).await;
            }
            // Waves timer
            tokio::time::sleep(Duration::from_secs(args.waves_pause as u64)).await;
        }
    }
}

#[async_trait]
impl AttackStrategy for RampUpStrategy {
    fn name(&self) -> AttackStrategyType {
        AttackStrategyType::RampUp
    }

    async fn run(self: Arc<Self>, args: &Args) {
        for _wave in 0..args.waves_amount {
            for i in 0..args.connections {
                let url = args.url.clone();
                let strat = Arc::clone(&self);

                let mut interval = interval(Duration::from_secs(args.connection_duration as u64));
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
                                        let (proceed, message) = strat.handle_messages(msg, i);

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
                // New connection timer
                tokio::time::sleep(Duration::from_millis(args.connection_pause as u64)).await;
            }
            // Waves timer
            tokio::time::sleep(Duration::from_secs(args.waves_pause as u64)).await;
        }
    }
}
