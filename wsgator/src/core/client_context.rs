use std::time::Duration;

use crate::core::behaviour::Behaviour;
use crate::core::monitor::Monitor;
use crate::Arc;
use futures::stream::SplitSink;
use futures::SinkExt;
use futures::StreamExt;
use tokio::net::TcpStream;
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
    stop_reciever: WatchReceiver<bool>,
    behaviour: Arc<Box<dyn Behaviour>>,
    monitor: Arc<Monitor>,
}

impl ClientContext {
    pub fn new(
        id: u32,
        url: String,
        stop_reciever: WatchReceiver<bool>,
        behaviour: Arc<Box<dyn Behaviour>>,
        monitor: Arc<Monitor>,
    ) -> Self {
        Self {
            id,
            url,
            stop_reciever,
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
        self.start_writer(sink);

        // Starting loop
        // Creating real web_socket connection (all)
        // Creating writer (to ones who need it) (considered in behaviour?)

        Ok(())
    }

    // Writer - responsible for acting as an Actor - collecting messages from others
    pub fn start_writer(
        &self,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    ) {
        tokio::spawn(async move {
            sink.send(Message::text("Task Started...")).await?;
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<(), WsGatorError>(())
        });
    }
    pub async fn prepare_taks(&self) {}
    pub async fn shutdown(&self) {}
}
