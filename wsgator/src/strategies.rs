use crate::Args;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use std::time::Duration;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};

// Enumiration for TypeChecking while getting user input from CLI
#[derive(clap::ValueEnum, Clone, Copy)]
pub enum AttackStrategyType {
    Flat,
    RampUp,
}

#[async_trait]
pub trait AttackStrategy {
    fn name(&self) -> AttackStrategyType;
    async fn run(&self, args: &Args);
}

pub struct FlatStrategy;
pub struct RampUpStrategy;

#[async_trait]
impl AttackStrategy for FlatStrategy {
    fn name(&self) -> AttackStrategyType {
        AttackStrategyType::Flat
    }
    async fn run(&self, args: &Args) {
        for i in 0..args.connections {
            let url = args.url.clone();

            let mut interval = interval(Duration::from_secs(12));
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
                                    match msg {
                                        Some(Ok(message)) => {
                                            match message {
                                                Message::Ping(bytes) => {
                                                    if let Err(e) = ws.send(Message::Pong(bytes)).await {
                                                        println!("Connection {}: Can't send Pong message: {}", i, e);
                                                        break;
                                                    }
                                                },
                                                _ => {},
                                            }
                                        },
                                        Some(Err(e)) => {
                                            println!("Connection {}: Recieved Err inside a message on listening to next message: {}", i, e);
                                            break;
                                        }
                                        None => {
                                            println!("Connection {}: Recieved None on listening to next message", i);
                                            break;
                                        }
                                    }
                                },
                                _ = interval.tick() => {
                                    println!("Connection {}: Reached its target time. Sending Close Frame", i);

                                    if let Err(e) = ws.send(Message::Close(None)).await {
                                        println!("Connection: {}: Can't send Close Frame: {}", i, e);
                                        break;
                                    }
                                    break;

                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("Connection {}: Error on WS connection: {}", i, e);
                        return;
                    }
                }
            });
        }
    }
}

#[async_trait]
impl AttackStrategy for RampUpStrategy {
    fn name(&self) -> AttackStrategyType {
        AttackStrategyType::RampUp
    }

    async fn run(&self, args: &Args) {}
}
