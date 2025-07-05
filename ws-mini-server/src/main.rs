use futures::SinkExt;
use futures::StreamExt;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let counter = Arc::new(AtomicU32::new(0));

    let listener = TcpListener::bind("127.0.0.1:9001").await?;

    println!("🚀 Server listening on ws://127.0.0.1:9001");

    while let Ok((stream, _)) = listener.accept().await {
        counter.fetch_add(1, Ordering::Relaxed);

        println!(
            "Starting connection. Current number is: {} <-----",
            counter.load(Ordering::Relaxed)
        );

        match accept_async(stream).await {
            Ok(ws_stream) => {
                println!("New connection found!");
                tokio::spawn(handle_ws(ws_stream, counter.clone()));
            }
            Err(e) => {
                println!("WebSocket connection Error: {e}");
                continue;
            }
        }
    }

    Ok(())
}

async fn handle_ws(ws_stream: WebSocketStream<tokio::net::TcpStream>, counter: Arc<AtomicU32>) {
    let (mut write, mut read) = ws_stream.split();
    let (msg_tx, mut msg_rx) = mpsc::channel::<Message>(1);
    let (pong_tx, mut pong_rx) = mpsc::channel::<bool>(1);
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    // Message writer task
    let writer_task = tokio::spawn({
        let mut shutdown_rx = shutdown_rx.clone();
        async move {
            loop {
                tokio::select!(
                    Some(msg) = msg_rx.recv() => {
                        if write.send(msg)
                            .await
                            .is_err() {
                                break;
                            }
                    },
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                )
            }
        }
    });

    // Message sender task
    let reciever_task = tokio::spawn({
        let msg_tx = msg_tx.clone();
        let shutdown_tx = shutdown_tx.clone();
        let mut shutdown_rx = shutdown_rx.clone();

        async move {
            loop {
                tokio::select!(
                    Some(message) = read.next() => {
                        match message {
                            Ok(Message::Text(txt)) => {
                                println!("Message recieved: {txt}");
                                //if let Err(_e) = msg_tx
                                //    .send(Message::Text(format!("Server response: {}", txt).into()))
                                //    .await { break; }
                            }
                            Ok(Message::Ping(data)) => {
                                println!("Ping recieved!");
                                if let Err(_e) = msg_tx
                                    .send(Message::Pong(data))
                                    .await { break; }
                            }

                            Ok(Message::Pong(_data)) => {
                                println!("Pong Recieved!");
                                if let Err(_e) = pong_tx
                                    .send(true)
                                    .await { break; }
                            }

                            Ok(Message::Close(_)) => {
                                println!("Client closed connection");
                                let _ = shutdown_tx.send(true);
                                break;
                            }

                            _ => {}
                        }
                    },

                    _ = shutdown_rx.changed() => {
                        break;
                    }
                );
            }
        }
    });

    let ping_task = tokio::spawn({
        let msg_tx = msg_tx.clone();
        let shutdown_tx = shutdown_tx.clone();

        async move {
            let mut inteval = tokio::time::interval(Duration::from_secs(30));
            loop {
                tokio::select! (

                _ = inteval.tick() => {
                    println!("Sending ping!");
                    if let Err(_e) = msg_tx
                        .send(Message::Ping("ping".into()))
                        .await { break; }

                    let pong_timer =
                        tokio::time::timeout(Duration::from_secs(60), pong_rx.recv()).await;

                    match pong_timer {
                        Ok(_) => {
                            continue;
                        }

                        _ => {
                            let _ = shutdown_tx.send(true);
                            println!("Ew! Dead connection - no Pong response... closing!");
                            break;
                        }
                }},

                _ = shutdown_rx.changed() => {
                    break;
                }


                )
            }
        }
    });

    tokio::select!(
        _ = ping_task => {},
        _ = reciever_task => {},
        _ = writer_task => {},
    );

    let _ = shutdown_tx.send(true);
    drop(msg_tx);
    println!("Ok... another connection closed!");

    counter.fetch_sub(1, Ordering::Relaxed);

    println!(
        "------> CLOSED CONNECTION. Current number is: {} <-----",
        counter.load(Ordering::Relaxed)
    );
}
