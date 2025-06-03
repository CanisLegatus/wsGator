use crate::AttackStrategy;
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

    async fn handle_special_events(
        &self,
        writer_tx: MpscSender<Message>,
    ) -> Result<(), WsGatorError> {
        // This is extremely bad one - I need to completely redo this
        loop {
            tokio::time::sleep(Duration::from_millis(self.spam_pause)).await;
            writer_tx
                .send(Message::Text("SPAM MACHINE!".into()))
                .await
                .map_err(|e| WsGatorError::MpscChannel(e.into()))?;
        }
        Ok(())
    }
}
