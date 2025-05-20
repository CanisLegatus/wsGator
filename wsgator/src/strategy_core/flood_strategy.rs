use crate::AttackStrategy;
use crate::CommonConfig;
use async_trait::async_trait;
use futures::SinkExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::watch::Receiver as WatchReceiver;
use futures::stream::SplitStream;
use tokio::time::Duration;

use tokio_tungstenite::tungstenite::Error as WsError;

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
        stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        stop_signal_receiver: WatchReceiver<Message>, 
        message_writer_sender: MpscSender<Message>,
        rx: Option<WatchReceiver<bool>>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>> {
        let spam_pause = self.spam_pause;

        Box::pin(async move {
            let mut rx = match rx {
                Some(rx) => rx,
                None => return Ok(()),
            };
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

                    // Spamming with constant messages
                    _ = tokio::time::sleep(Duration::from_millis(spam_pause)) => {
                       ws.send(Message::Text(format!("SPAM! From connection {}", i).into())).await?;
                    }

                    _ = rx.changed() => {
                        println!("Connection {}: Reached its target time. Dropping connection", i);
                        ws.send(Message::Close(None)).await?;
                        break Ok(());
                    }
                }
            }
        })
    }
}
