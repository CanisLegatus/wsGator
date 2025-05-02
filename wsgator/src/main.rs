use std::time::Duration;

use clap::Parser;
use futures::SinkExt;
use tokio_tungstenite::connect_async;

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

                    // send some text
                    ws.send("Hello".into()).await.expect("Can't send it");

                    // Connection stable
                    loop {
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        if let Err(_e) = ws.send("Ping".into()).await {
                            println!("Closing connection {}", i);
                            break;
                        }
                    }
                }

                Err(e) => println!("Connection {} failed: {}", i, e),
            }
        });
    }
    tokio::signal::ctrl_c().await.unwrap();
}
