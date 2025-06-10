use crate::AttackStrategy;
use std::pin::Pin;
use crate::CommonConfig;
use crate::core::error::WsGatorError;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio_tungstenite::tungstenite::Message;

pub struct FloodStrategy {
    pub common_config: Arc<CommonConfig>,
    pub spam_pause: u64,
}

#[async_trait]
impl AttackStrategy for FloodStrategy {
    fn get_common_config(&self) -> Arc<CommonConfig> {
        self.common_config.clone()
    }

    fn prepare_special_events(
        self: Arc<Self>,
        writer_tx: MpscSender<Message>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>> {
        Box::pin(async move {

            loop{
                tokio::time::sleep(Duration::from_millis(self.spam_pause)).await;
                writer_tx
                    .send(Message::Text("SPAM".into()))
                    .await
                    .map_err(|e| WsGatorError::MpscChannel(e.into()))?;
            }
        })
    }
}
