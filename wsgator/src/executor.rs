use crate::AttackStrategy;
use futures::SinkExt;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
use tokio::time::Duration;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::{connect_async, tungstenite::Message};
pub struct Executor;

type ConnectionTaskFuture = Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>>;

impl Executor {
    pub async fn run(
        &self,
        strategy: Arc<dyn AttackStrategy + Send + Sync>,
    ) -> Result<(), WsError> {
        let config = strategy.get_common_config();

        for _wave in 0..config.waves_number {
            let con = Arc::clone(&config);
            // Creating independent watch_channel to stop all tasks extenally
            let mut watch_channel: Option<Receiver<bool>> = None;
            let timer_task = if config.external_timer {
                let (stop_tx, stop_rx) = watch::channel(false);
                watch_channel = Some(stop_rx);
                Some(Box::pin(async move {
                    tokio::time::sleep(Duration::from_secs(con.connection_duration)).await;
                    let _ = stop_tx.send(true);
                }))
            } else {
                None
            };

            let mut tasks: Vec<ConnectionTaskFuture> = vec![];

            // Creating connections
            for i in 0..config.connection_number {
                let strategy = Arc::clone(&strategy);
                let con = Arc::clone(&config);
                let rx = watch_channel.clone();

                // Getting websocket connection
                let mut ws = strategy.get_ws_connection(&con.url_under_fire).await?;

                // Saying hello to connection
                ws.send(Message::Text(format!("Peer {} saying Hello!", i).into()))
                    .await?;

                // Spawning thread for each connection
                tasks.push(strategy.run_connection_loop(ws, rx, con, i));
            }

            // Waves logic //
            // Spawning collected tasks
            for task in tasks {
                tokio::time::sleep(Duration::from_millis(config.connection_pause)).await;
                tokio::spawn(task);
            }

            // Spawning timer
            if let Some(timer_task) = timer_task {
                tokio::spawn(timer_task);
            }

            // Waves delay timer
            tokio::time::sleep(Duration::from_secs(config.waves_pause)).await;
        }
        Ok(())
    }
}
