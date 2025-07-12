use crate::core::error::WatchChannelError;
use crate::core::error::WsGatorError;
use crate::core::error_log::ErrorLog;
use crate::core::executor::Executor;
use crate::CommonConfig;
use async_trait::async_trait;
use futures::future;
use futures::stream;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use futures::SinkExt;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver as MpscReceiver;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::watch;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio::task::JoinSet;
use tokio::time::Duration;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;

pub type TasksVector =
    Vec<Result<Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>>, WsError>>;

pub type TasksRunner =
    Pin<Box<dyn Future<Output = Result<JoinSet<Result<(), WsGatorError>>, WsGatorError>> + Send>>;

pub type TimerTask = Pin<Box<dyn Future<Output = Result<(), WatchChannelError>> + Send>>;

pub type ConnectionTaskFuture =
    Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send + 'static>>;

#[async_trait]
pub trait AttackStrategy: Send + Sync {
    fn get_common_config(&self) -> Arc<CommonConfig>;

    async fn prepare_strategy(
        self: Arc<Self>,
        config: Arc<CommonConfig>,
        log: Arc<ErrorLog>,
    ) -> Result<TasksRunner, WsGatorError>
    where
        Self: 'static,
    {
        let (stop_rx, timer) = self.get_timer_task(config.clone());

        // Creating tasks
        let tasks = self
            .clone()
            .get_connections(config.clone(), stop_rx)
            .await?;

        // Getting run logic
        let runner = self.get_runner(tasks, timer, config, log);

        Ok(runner)
    }

    // Getting not yet running connections
    async fn get_connections(
        self: Arc<Self>,
        config: Arc<CommonConfig>,
        stop_rx: WatchReceiver<bool>,
    ) -> Result<Vec<Result<ConnectionTaskFuture, WsError>>, WsError>
    where
        Self: 'static,
    {
        let tasks: Vec<Result<ConnectionTaskFuture, WsError>> =
            stream::iter(0..config.connection_number)
                .map(|i| {
                    let con = Arc::clone(&config);
                    let stop_rx = stop_rx.clone();
                    let strat = self.clone();

                    async move {
                        // Returning future from strategy
                        Ok(strat.get_task(con.url_under_fire.clone(), stop_rx, i))
                    }
                })
                .buffer_unordered(1000)
                .collect::<Vec<Result<ConnectionTaskFuture, WsError>>>()
                .await;

        Ok(tasks)
    }

    // Creating strategy-specific timer
    fn get_timer_task(&self, config: Arc<CommonConfig>) -> (WatchReceiver<bool>, TimerTask) {
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(config.connection_duration)).await;
            stop_tx.send(true)?;
            Ok(())
        });
        (stop_rx, task)
    }

    // Creating task-runner default logic
    fn get_runner(
        &self,
        tasks: TasksVector,
        timer: TimerTask,
        config: Arc<CommonConfig>,
        log: Arc<ErrorLog>,
    ) -> TasksRunner {
        Box::pin(async move {
            let mut join_set = JoinSet::new();

            // Spawning an external stop timer
            // Timer will destroy itself if all stop_rx will be dropped
            tokio::spawn(timer);

            for task in tasks {
                match task {
                    Ok(task) => {
                        tokio::time::sleep(Duration::from_millis(config.connection_pause)).await;
                        join_set.spawn(task);
                    }
                    Err(e) => {
                        log.count(e.into());
                    }
                }
            }

            Ok(join_set)
        })
    }

    // Creating defatult special task
    fn prepare_special_events(
        self: Arc<Self>,
        _: MpscSender<Message>,
        mut stop_rx: WatchReceiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>> {
        Box::pin(async move {
            tokio::select! {
                _ = future::pending() => {
                    Ok(())
                },
                _ = stop_rx.changed() => {
                    Ok(())
                }
            }
        })
    }

    fn get_writer(
        &self,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut writer_rx: MpscReceiver<Message>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>> {
        Box::pin(async move {
            while let Some(message) = writer_rx.recv().await {
                sink.send(message).await?;
            }
            Ok(())
        })
    }

    fn get_task(
        self: Arc<Self>,
        url: String,
        mut stop_rx: WatchReceiver<bool>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send + 'static>>
    where
        Self: 'static,
    {
        Box::pin(async move {
            let ws = Executor::get_ws_connection(&url).await?;
            let (sink, mut stream) = ws.split();
            let (writer_tx, writer_rx) = mpsc::channel::<Message>(128);
            let writer = self.get_writer(sink, writer_rx);

            // Spawning parallel tasks
            let writer_handler = tokio::spawn(writer);
            let special_event_handle = tokio::spawn(
                self.clone()
                    .prepare_special_events(writer_tx.clone(), stop_rx.clone()),
            );

            // Main logic default loop of a task
            let result = loop {
                tokio::select! {
                    result = self.run_base_events(&mut stream, &writer_tx, i) => {
                        match result {
                            Ok(false) => {
                                drop(writer_tx);
                                break Ok(());
                            },
                            Ok(true) => {},
                            Err(e) => {
                                drop(writer_tx);
                                break Err(e);
                            }
                        }
                    }
                    result = stop_rx.changed() => {
                        result.map_err(|e| WsGatorError::WatchChannel(e.into()))?;
                        drop(writer_tx);
                        break Ok(());
                    }
                }
            };

            // Awaiting for everyone to stop
            special_event_handle.await??;
            writer_handler.await??;

            result
        })
    }

    async fn run_base_events(
        &self,
        stream: &mut SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        writer_tx: &MpscSender<Message>,
        i: u32,
    ) -> Result<bool, WsGatorError> {
        match stream.next().await {
            Some(msg) => {
                let (proceed, message_opt) = self
                    .handle_messages(msg, i)
                    .map_err(WsGatorError::WsError)?;
                if let Some(message) = message_opt {
                    writer_tx
                        .send(message.clone())
                        .await
                        .map_err(|e| WsGatorError::MpscChannel(e.into()))?;
                }
                Ok(proceed)
            }
            None => Ok(false),
        }
    }

    fn handle_messages(
        &self,
        msg: Result<Message, tokio_tungstenite::tungstenite::Error>,
        connection_number: u32,
    ) -> Result<(bool, Option<Message>), Box<WsError>> {
        match msg {
            Ok(message) => match message {
                Message::Ping(bytes) => Ok((true, Some(Message::Pong(bytes)))),
                Message::Text(_txt) => Ok((true, None)),
                Message::Close(_) => Ok((false, None)),

                _ => Ok((true, None)),
            },
            Err(e) => {
                println!(
                    "Connection {connection_number}: Recieved Err inside a message on listening to next message: {e}",
                );
                Err(Box::new(e))
            }
        }
    }
}
