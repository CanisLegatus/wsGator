use crate::{AttackStrategy, CommonConfig};
use futures::SinkExt;
use futures::stream;
use futures::stream::StreamExt;
use tokio::task::JoinSet;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::time::Duration;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::{connect_async, tungstenite::Message};
pub struct Executor;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;

use super::error::WatchChannelError;
use super::error::WsGatorError;
use super::error_log;
use super::error_log::ErrorLog;
type ConnectionTaskFuture = Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send + 'static>>;

impl Executor {
    async fn get_ws_connection(
        &self,
        url: &str,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let (ws, _) = connect_async(url).await?;
        Ok(ws)
    }

    pub fn get_timer_task(
        config: Arc<CommonConfig>,
    ) -> (WatchReceiver<bool>, Pin<Box<impl Future<Output = Result<(), WatchChannelError>>>>) {
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(config.connection_duration)).await;
            stop_tx.send(true)?;
            Ok(())
        });
        (stop_rx, task)
    }

    pub async fn get_connections(
        &self,
        strategy: Arc<dyn AttackStrategy + Send>,
        config: Arc<CommonConfig>,
        stop_rx: WatchReceiver<bool>,
    ) -> Result<Vec<ConnectionTaskFuture>, WsError> {
        let tasks: Vec<ConnectionTaskFuture> = stream::iter(0..config.connection_number)
            .map(|i| {
                let strategy = Arc::clone(&strategy);
                let con = Arc::clone(&config);
                let stop_rx = stop_rx.clone();

                async move {
                    let ws = match self.get_ws_connection(&con.url_under_fire).await {
                        Ok(mut ws) => {
                            // Sending hello
                            if let Err(e) =
                                ws.send(Message::Text(format!("Peer {}", i).into())).await
                            {
                                println!("Send err: {}, On connection: {}", e, i);
                                return None;
                            }
                            ws
                        }
                        Err(e) => {
                            println!("Connection failed: {}", e);
                            return None;
                        }
                    };

                    // Returning future from strategy
                    Some(strategy.get_task(ws, stop_rx, i))
                }
            })
            .buffer_unordered(100)
            .filter_map(|task| async move { task })
            .collect::<Vec<ConnectionTaskFuture>>()
            .await;

        Ok(tasks)
    }

    pub async fn run(
        &self,
        strategy: Arc<dyn AttackStrategy + Send + Sync>,
        log: Arc<ErrorLog>,
    ) -> Result<(), WsError> {
        let config = strategy.get_common_config();

        for _wave in 0..config.waves_number {
            let con = Arc::clone(&config);
            let mut join_set = JoinSet::new();

            // Creating independent watch_channel to stop all tasks extenally
            let (watch_channel, timer_task) = Self::get_timer_task(con);
            let tasks = self
                .get_connections(strategy.clone(), config.clone(), watch_channel)
                .await?;

            // Waves logic
            // Spawning collected tasks
            for task in tasks {
                tokio::time::sleep(Duration::from_millis(config.connection_pause)).await;
                join_set.spawn(task);
            }

            // Spawning timer
            tokio::spawn(timer_task);
            
            while let Some(res) = join_set.join_next().await {
                match res {
                    Ok(Ok(())) => continue,
                    Ok(Err(e)) => {
                        match e {
                            WsGatorError::WsError(inner) => {
                                log.count(inner);
                            },
                            _=> {  }
                        }
                    },
                    Err(join_error) => {
                        println!("Join Error! Error: {}", join_error);
                    },
                }
            }

            // Waves delay timer
            tokio::time::sleep(Duration::from_secs(config.waves_pause)).await;
        }
        Ok(())
    }
}
