use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tokio_tungstenite::MaybeTlsStream;
use std::pin::Pin;
use crate::AttackStrategy;
use crate::CommonConfig;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;
use futures::StreamExt;
use futures::SinkExt;
use tokio_tungstenite::tungstenite::Message;

use tokio_tungstenite::tungstenite::Error as WsError;

pub struct RampUpStrategy {
    pub common_config: Arc<CommonConfig>,
}

#[async_trait]
impl AttackStrategy for RampUpStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }
    fn run_connection_loop(
        self: Arc<Self>,
        mut ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        rx: Option<Receiver<bool>>,
        _config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + Sync + 'static>> {
        //let mut interval = interval(Duration::from_secs(config.connection_duration));
        //interval.tick().await;

        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return Ok(()),
            };
            // Connection loop
            loop {
                tokio::select! {
                    msg = ws.next() => {
                        let (proceed, message) = self.handle_messages(msg, i);

                        if let Some(message) = message {
                            ws.send(message).await?;
                        }

                        if !proceed {
                            break Ok(());
                        }
                    },

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
