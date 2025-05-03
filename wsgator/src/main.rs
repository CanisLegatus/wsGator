use std::time::Duration;

use clap::Parser;
use futures::SinkExt;
use futures::StreamExt;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value = "ws://localhost:9001")]
    url: String,

    #[clap(short, long, default_value = "100")]
    connections: usize,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    for i in 0..args.connections {
        let url = args.url.clone();

        let _ = tokio::time::sleep(Duration::from_millis(1)).await;

        tokio::spawn(async move {
            match connect_async(&url).await {
                Ok((mut ws, _)) => {
                    println!("Connection {}: OK", i);

                    // Sending initial text
                    if let Err(e) = ws.send(Message::Text("Hello from thread!".into())).await {
                        println!("Can't send hello! Connection: {} Error: {}", i, e);
                        return;
                    }

                    let mut interval = interval(Duration::from_secs(10));
                    interval.tick().await;

                    // Connection stable
                    loop {
                        tokio::select! {
                            msg = ws.next() => {
                                match msg {
                                    Some(Ok(message)) => {
                                        match message {
                                            Message::Text(text) => {
                                                println!("Conntection {}: Recieved text -> {}", i, text);
                                            }
                                            Message::Ping(payload) => {
                                                println!("Connection {}: Recieved ping with payload", i);
                                                if let Err(e) = ws
                                                    .send(Message::Pong(payload))
                                                    .await {
                                                        println!("Connection {}: Failed to send Pong", i);
                                                        break; }
                                            }
                                            Message::Binary(_binary) => {
                                                println!("Connection {}: Recieved binary", i);
                                            }
                                            Message::Pong(_payload) => {
                                                println!("Connection {}: Recieved Pong", i);
                                            }
                                            Message::Close(_) => {
                                                println!("Connection {}: Close Frame recieved", i);
                                                break;
                                            }
                                            Message::Frame(_) => {
                                                println!("Connection {}: Recieved a Frame", i);
                                            }
                                        }
                                    },

                                    Some(Err(e)) => {
                                        println!("Connection {}: WebSocket error: {}", i, e);
                                        break;
                                    },

                                    None => {
                                        println!("Connection {}: Connection closed", i);
                                        break;
                                    }
                                }
                            }
                            _ = interval.tick() => {
                                if let Err(e) = ws.send(Message::Ping(b"ping".to_vec().into()))
                                    .await {
                                        println!("Connection {}: Failed to send ping with error: {}", i, e);
                                        break
                                }
                            }
                        }
                    }
                }

                Err(e) => println!("Connection {} failed: {}", i, e),
            }
        });
    }
    tokio::signal::ctrl_c().await.unwrap();
}
