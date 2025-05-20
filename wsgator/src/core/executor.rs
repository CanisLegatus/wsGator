use crate::{AttackStrategy, CommonConfig};
use tokio::sync::mpsc;
use futures::{SinkExt, StreamExt};
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

type ConnectionTaskFuture = Pin<Box<dyn Future<Output = Result<(), WsError>> + Send + 'static>>;

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
    ) -> (
        Option<WatchReceiver<bool>>,
        Option<Pin<Box<impl Future<Output = ()>>>>,
    ) {
        if config.external_timer {
            let (stop_tx, stop_rx) = watch::channel(false);
            let task = Box::pin(async move {
                tokio::time::sleep(Duration::from_secs(config.connection_duration)).await;
                let _ = stop_tx.send(true);
            });
            (Some(stop_rx), Some(task))
        } else {
            (None, None)
        }
    }

    pub async fn get_connections(
        &self,
        strategy: Arc<dyn AttackStrategy + Send>,
        config: Arc<CommonConfig>,
        watch_channel: Option<WatchReceiver<bool>>,
    ) -> Result<Vec<ConnectionTaskFuture>, WsError> {
        let mut tasks: Vec<ConnectionTaskFuture> = vec![];

        // Creating connections
        for i in 0..config.connection_number {
            let strategy = Arc::clone(&strategy);
            let con = Arc::clone(&config);
            let stop_rx = watch_channel.clone();

            // Getting websocket connection
            let ws = self.get_ws_connection(&con.url_under_fire).await?;
            let (mut sink, stream) = ws.split();
            
            // Getting mpsc task to send messages to writer
            let (writer_tx, writer_rx) = mpsc::channel::<Message>(8);

            // Saying hello to connection
            sink.send(Message::Text(format!("Peer {} saying Hello!", i).into()))
                .await?;
            
            strategy.run_writer(sink, writer_rx).await;

            // Spawning thread for each connection
            tasks.push(strategy.run_connection_loop(stream, stop_rx, writer_tx, con, i));
        }
        Ok(tasks)
    }

    pub async fn run(
        &self,
        strategy: Arc<dyn AttackStrategy + Send + Sync>,
    ) -> Result<(), WsError> {
        let config = strategy.get_common_config();

        for _wave in 0..config.waves_number {
            let con = Arc::clone(&config);

            // Creating independent watch_channel to stop all tasks extenally
            let (watch_channel, timer_task) = Self::get_timer_task(con);
            let tasks = self
                .get_connections(strategy.clone(), config.clone(), watch_channel)
                .await?;

            // Waves logic
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
