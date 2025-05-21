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
}
