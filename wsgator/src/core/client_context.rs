use crate::Arc;
use crate::core::behaviour::Behaviour;
use crate::core::monitor::Monitor;
use futures::SinkExt;
use futures::StreamExt;
use futures::stream::SplitSink;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender as MpscSender;
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
    url: String,
    id: u32,
    writer_sender: MpscSender<Message>,
    stop_reciever: WatchReceiver<bool>,
    behaviour: Arc<dyn Behaviour>,
    monitor: Arc<Monitor>,
}

impl ClientContext {
    pub fn new(
        url: String,
        id: u32,
        writer_sender: MpscSender<Message>,
        stop_reciever: WatchReceiver<bool>,
        behaviour: Arc<dyn Behaviour>,
        monitor: Arc<Monitor>,
    ) -> Self {
        Self {
            url,
            id,
            writer_sender,
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

        // Starting loop

        Ok(())
    }
    pub async fn start_writer(
        &self,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    ) {
        tokio::spawn(async move {
            sink.send(Message::text("LOl")).await?;
            Ok::<(), WsGatorError>(())
        });
    }
    pub async fn shutdown(&self) {}
}
