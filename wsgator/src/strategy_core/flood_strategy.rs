use crate::AttackStrategy;
use crate::CommonConfig;
use crate::core::error::WsGatorError;
use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::time::interval;
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
        mut stop_rx: WatchReceiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>> {
        Box::pin(async move {
            let mut interval = interval(Duration::from_millis(self.spam_pause));
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        writer_tx
                            .send(Message::Text("SPAM".into()))
                            .await
                            .map_err(|e| WsGatorError::MpscChannel(e.into()))?;
                    },

                    _ = stop_rx.changed() => {
                        break;
                    }
                }
            }
            Ok(())
        })
    }
}
