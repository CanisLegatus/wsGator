use crate::Arc;
use crate::core::behaviour::Behaviour;
use crate::core::monitor::Monitor;
use futures::SinkExt;
use futures::StreamExt;
use futures::stream::SplitSink;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver as MpscReceiver;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::error::WsGatorError;

// Client Context
// One connection (connection - recieve of message)
// Usage of behaviour to react on events
// Sending metrics to Monitor
// Collecting Errors

pub struct ClientContext {
    id: u32,
    url: String,
    stop_rx: WatchReceiver<bool>,
    behaviour: Arc<Box<dyn Behaviour>>,
    monitor: Arc<Monitor>,
}

impl ClientContext {
    pub fn new(
        id: u32,
        url: String,
        stop_rx: WatchReceiver<bool>,
        behaviour: Arc<Box<dyn Behaviour>>,
        monitor: Arc<Monitor>,
    ) -> Self {
        Self {
            id,
            url,
            stop_rx,
            behaviour,
            monitor,
        }
    }

    async fn get_ws_connection(
        &self,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let (ws, _) = connect_async(self.url.clone()).await?;
        Ok(ws)
    }

    pub async fn run(&mut self) -> Result<(), WsError> {
        // Starting our websocket
        let websocket = self.get_ws_connection().await?;
        let (sink, stream) = websocket.split();

        // Starting message channel
        let (message_tx, message_rx) = mpsc::channel::<Message>(128);

        // Starting writer
        let _ = self.start_writer(sink, message_rx);

        // Starting blocking behaviour cycle main_thing (really blocking or spawning?)
        let _ = self.behaviour.run(self.id, stream, message_tx, self.stop_rx.clone()).await;

        Ok(())
    }

    // Writer - responsible for acting as an Actor - collecting messages from others
    pub fn start_writer(
        &self,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut message_rx: MpscReceiver<Message>,
    ) -> Result<(), WsGatorError> {
        tokio::spawn(async move {
            while let Some(message) = message_rx.recv().await {
                sink.send(message).await?;
            }

            Ok::<(), WsGatorError>(())
        });

        Ok(())
    }
    pub async fn prepare_taks(&self) {}
    pub async fn shutdown(&self) {}
}
