use clap::Parser;
use futures::{SinkExt, StreamExt};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinSet,
    time::timeout,
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Error, Message},
};
use tokio_util::sync::CancellationToken;

/// Local WebSocket echo server for wsGator smoke tests.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(long, default_value = "127.0.0.1:9001")]
    bind: SocketAddr,
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let listener = TcpListener::bind(args.bind).await?;
    println!("Echo server listening on ws://{}", listener.local_addr()?);
    let stop = CancellationToken::new();
    let mut tasks = JoinSet::new();
    let mut accepted = 0u64;
    loop {
        tokio::select! {
            signal = tokio::signal::ctrl_c() => { signal?; break; }
            result = listener.accept() => {
                let (stream, _) = result?;
                accepted += 1;
                tasks.spawn(handle(stream, stop.clone()));
            }
            Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                match result {
                    Ok(Ok(())) => (),
                    Ok(Err(e)) => eprintln!("Connection: {e}"),
                    Err(e) => eprintln!("Task: {e}"),
                }
            }
        }
    }
    stop.cancel();
    while tasks.join_next().await.is_some() {}
    println!("Stopped; accepted {accepted} TCP connections");
    Ok(())
}
async fn handle(stream: TcpStream, stop: CancellationToken) -> Result<(), String> {
    let mut ws = tokio::select! {
        _ = stop.cancelled() => return Ok(()),
        result = timeout(Duration::from_secs(5), accept_async(stream)) => result.map_err(|_| "handshake timeout")?.map_err(|e| e.to_string())?,
    };
    loop {
        tokio::select! {
            _ = stop.cancelled() => {
                let _ = timeout(Duration::from_secs(2), ws.close(None)).await;
                return Ok(());
            }
            message = ws.next() => {
                let reply = match message {
                    Some(Ok(m @ (Message::Text(_) | Message::Binary(_)))) => Some(m),
                    Some(Ok(Message::Ping(_))) => None,
                    Some(Ok(Message::Close(_))) => {
                        let _ = timeout(Duration::from_secs(2), ws.flush()).await;
                        return Ok(());
                    }
                    None | Some(Err(Error::ConnectionClosed)) => return Ok(()),
                    Some(Err(e)) => return Err(e.to_string()),
                    _ => continue,
                };
                tokio::select! {
                    _ = stop.cancelled() => return Ok(()),
                    result = timeout(Duration::from_secs(5), async {
                        if let Some(m) = reply { ws.send(m).await } else { ws.flush().await }
                    }) => result.map_err(|_| "write timeout")?.map_err(|e| e.to_string())?,
                }
            }
        }
    }
}
