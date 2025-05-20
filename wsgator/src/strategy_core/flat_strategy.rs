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
        stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        stop_rx: Option<WatchReceiver<bool>>, 
        writer_tx: MpscSender<Message>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>> {
        Box::pin(async move {
            let mut rx = match stop_rx {
                Some(rx) => rx,
                None => return Ok(()),
            };
            // Connection loop
            loop {
                tokio::select! {

                    //result = self.handle_base_events(ws, i) => {},

                    _ = rx.changed() => {
                        println!("Connection {}: Reached its target time. Sending Close Frame", i);
                        writer_tx.send(Message::Close(None));
                        break Ok(());
                    }
                }
            }
        })
    }
}
