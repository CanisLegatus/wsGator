use crate::{AttackStrategy, CommonConfig};
use futures::stream;
use futures::stream::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
pub struct Executor;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;

use super::error::WatchChannelError;
use super::error::WsGatorError;
use super::error_log::ErrorLog;
type ConnectionTaskFuture =
    Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send + 'static>>;

impl Executor {
    pub async fn get_ws_connection(
        url: &str,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let (ws, _) = connect_async(url).await?;
        Ok(ws)
    }

    pub fn get_timer_task(
        config: Arc<CommonConfig>,
    ) -> (
        WatchReceiver<bool>,
        Pin<Box<impl Future<Output = Result<(), WatchChannelError>>>>,
    ) {
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(config.connection_duration)).await;
            stop_tx.send(true)?;
            Ok(())
        });
        (stop_rx, task)
    }

    // Getting not yet running connections
    pub async fn get_connections(
        &self,
        strategy: Arc<dyn AttackStrategy + Send>,
        config: Arc<CommonConfig>,
        stop_rx: WatchReceiver<bool>,
    ) -> Result<Vec<Result<ConnectionTaskFuture, WsError>>, WsError> {
        let tasks: Vec<Result<ConnectionTaskFuture, WsError>> =
            stream::iter(0..config.connection_number)
                .map(|i| {
                    let strategy = Arc::clone(&strategy);
                    let con = Arc::clone(&config);
                    let stop_rx = stop_rx.clone();

                    async move {
                        // Returning future from strategy
                        Ok(strategy.get_task(con.url_under_fire.clone(), stop_rx, i))
                    }
                })
                .buffer_unordered(1000)
                .collect::<Vec<Result<ConnectionTaskFuture, WsError>>>()
                .await;

        Ok(tasks)
    }

    // Main run function of a Executor - major logic is here
    pub async fn run(
        &self,
        strategy: Arc<dyn AttackStrategy + Send + Sync>,
        log: Arc<ErrorLog>,
    ) -> Result<(), WsGatorError> {
        let config = strategy.get_common_config();

        for _wave in 0..config.waves_number {
            let con = Arc::clone(&config);

            // Creating independent watch_channel to stop all tasks extenally
            let (stop_rx, timer_task) = Self::get_timer_task(con.clone());
            let tasks = self
                .get_connections(strategy.clone(), config.clone(), stop_rx)
                .await?;

            println!("--->> STARTING IN 3 SECONDS <<---");
            tokio::time::sleep(Duration::from_secs(3)).await;
            println!("--->>🐊 BITE! 🐊<<---");

            // Getting run logic
            let runner = strategy.get_start_logic(tasks, con, log.clone());

            // Running tasks and collecting them to join_set
            let mut join_set = runner.await?;

            // Spawning timer
            tokio::spawn(timer_task);

            // Handling errors in async tasks
            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok(Ok(())) => continue,
                    Ok(Err(e)) => {
                        log.count(e);
                    }
                    Err(join_error) => {
                        println!("Join Error! Error: {}", join_error);
                    }
                }
            }

            // Waves delay timer
            tokio::time::sleep(Duration::from_secs(config.waves_pause)).await;
        }
        Ok(())
    }
}
