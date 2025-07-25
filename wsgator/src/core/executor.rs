use crate::AttackStrategy;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::time::Duration;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;

use super::error::WsGatorError;
use super::error_log::ErrorLog;

pub struct Executor;

impl Executor {
    pub async fn get_ws_connection(
        url: &str,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let (ws, _) = connect_async(url).await?;
        Ok(ws)
    }

    // Main run function of a Executor - major logic is here
    // TODO - UNDER HEAVY CONSTRUCTION
    // Collect all modules (Behaviour, Runner, Monitor in ClientContext)
    // Connect it and run
    pub async fn run(
        &self,
        strategy: Arc<dyn AttackStrategy + Send + Sync>,
        log: Arc<ErrorLog>,
    ) -> Result<(), WsGatorError> {
        println!("--->> STARTING IN 3 SECONDS <<---");
        tokio::time::sleep(Duration::from_secs(3)).await;
        println!("--->>🐊 BITE! 🐊<<---");

        let config = strategy.get_common_config();

        for wave in 0..config.waves_number {
            println!("---> Wave PREP stage...");

            // Getting run logic
            let runner = strategy
                .clone()
                .prepare_strategy(config.clone(), log.clone())
                .await?;

            println!("---> Wave ATTACK stage...");

            // Running tasks and collecting them to join_set
            let mut join_set = runner.await?;

            // Handling errors in async tasks
            while let Some(result_of_async_task) = join_set.join_next().await {
                match result_of_async_task {
                    Ok(Ok(())) => continue,
                    Ok(Err(e)) => {
                        log.count(e);
                    }
                    Err(join_error) => {
                        println!("Join Error! Error: {join_error}");
                    }
                }
            }

            // Waves delay timer
            tokio::time::sleep(Duration::from_secs(config.waves_pause)).await;
            println!("---> Wave {} is completed!", wave + 1);
        }
        Ok(())
    }
}
