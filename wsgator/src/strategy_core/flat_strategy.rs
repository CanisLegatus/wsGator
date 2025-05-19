use crate::AttackStrategy;
use crate::CommonConfig;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::watch::Receiver;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use tokio_tungstenite::tungstenite::Error as WsError;

pub struct FlatStrategy {
    pub common_config: Arc<CommonConfig>,
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
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>> {
        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return Ok(()),
            };
            // Connection loop
            loop {
                tokio::select! {

                    result = self.handle_base_events(ws, i) => {},

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
