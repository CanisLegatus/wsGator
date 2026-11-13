use futures::{SinkExt, StreamExt};
use tokio::{
    net::TcpListener,
    task::{JoinHandle, JoinSet},
    time::{Duration, timeout},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

/// A real loopback WebSocket echo server, owned by the interactive application.
pub struct Demo {
    pub url: String,
    stop: CancellationToken,
    task: JoinHandle<()>,
}
impl Demo {
    pub async fn start() -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("Cannot start local demo: {e}"))?;
        let url = format!("ws://{}", listener.local_addr().map_err(|e| e.to_string())?);
        let stop = CancellationToken::new();
        let cancelled = stop.clone();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    _ = cancelled.cancelled() => break,
                    connection = listener.accept() => {
                        let Ok((stream,_)) = connection else { break; };
                        clients.spawn(async move {
                            let Ok(Ok(mut ws)) = timeout(Duration::from_secs(5),accept_async(stream)).await else { return; };
                            while let Some(Ok(message)) = ws.next().await {
                                let result = match message {
                                    m @ (Message::Text(_) | Message::Binary(_)) => ws.send(m).await,
                                    Message::Ping(_) => ws.flush().await,
                                    Message::Close(_) => { let _ = ws.flush().await; return; }
                                    _ => continue,
                                };
                                if result.is_err() { return; }
                            }
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => (),
                }
            }
            clients.abort_all();
            while clients.join_next().await.is_some() {}
        });
        Ok(Self { url, stop, task })
    }
    pub async fn shutdown(&mut self) {
        self.stop.cancel();
        let _ = (&mut self.task).await;
    }
}
impl Drop for Demo {
    fn drop(&mut self) {
        self.stop.cancel();
        self.task.abort();
    }
}
