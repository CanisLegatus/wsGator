use crate::core::behaviour;
use crate::Arc;
use crate::core::behaviour::Behaviour;
use crate::core::monitor::Monitor;
use futures::StreamExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

// Client Context
// One connection (connection - recieve of message)
// Usage of behaviour to react on events
// Sending metrics to Monitor
// Collecting Errors

pub struct ClientContext {
    id: u32,
    url: String,
    stop_rx: WatchReceiver<bool>,
    behaviour: Arc<dyn Behaviour>,
    monitor: Arc<Monitor>,
}

impl ClientContext {
    pub fn new(
        id: u32,
        url: String,
        stop_rx: WatchReceiver<bool>,
        behaviour: Arc<dyn Behaviour>,
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
        let writer_handle = self.behaviour.start_writer(sink, message_rx);
        let special_loop = self.behaviour.get_special_loop();
        let basic_loop = self.behaviour.get_basic_loop(stream, message_tx.clone());
        // Starting blocking behaviour cycle main_thing (really blocking or spawning?)
        
        // Lets start it!

       //tokio::spawn(special_loop);

        Ok(())
    }

    pub async fn prepare_taks(&self) {}
    pub async fn shutdown(&self) {}
}
