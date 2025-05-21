use crate::AttackStrategy;
use crate::CommonConfig;
use async_trait::async_trait;
use futures::SinkExt;
use futures::stream::SplitStream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::time::Duration;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

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
        mut stop_rx: WatchReceiver<bool>,
        writer_tx: MpscSender<Message>,
        config: Arc<CommonConfig>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>> {
        Box::pin(async move {
            // Connection loop
            loop {
                tokio::select! {

                    //result = self.handle_base_events(ws, i) => {},

                    _ = stop_rx.changed() => {
                        println!("Connection {}: Reached its target time. Sending Close Frame", i);
                        writer_tx.send(Message::Close(None)).await;
                        break Ok(());
                    }
                }
            }
        })
    }
}
