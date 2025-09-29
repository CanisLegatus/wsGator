use crate::Arc;
use crate::core::behaviour::Behaviour;
use crate::core::monitor::Monitor;
use crate::core::timer::Signal;
use crate::core::timer::SignalType;
use futures::StreamExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
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
    timer: Arc<Signal>,
    behaviour: Arc<dyn Behaviour>,
    monitor: Arc<Monitor>,
}

impl ClientContext {
    pub fn new(
        id: u32,
        url: String,
        timer: Arc<Signal>,
        behaviour: Arc<dyn Behaviour>,
        monitor: Arc<Monitor>,
    ) -> Self {
        Self {
            id,
            url,
            timer,
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

        // Adding on_connect
        self.on_connect().await;

        let (sink, stream) = websocket.split();

        // Starting message channel
        let (message_tx, message_rx) = mpsc::channel::<Message>(128);

        // Starting writer
        // TODO: collect all handles!
        let writer = self.behaviour.get_writer(sink, message_rx);
        let special_loop = self.behaviour.clone().get_special_loop();
        let basic_loop = self
            .behaviour
            .clone()
            .get_basic_loop(stream, message_tx.clone());

        // Lets start it!
        if let Some(writer) = writer {
            tokio::spawn(writer);
        }

        // If special loop present - starting it
        if let Some(special_loop) = special_loop {
            tokio::spawn(special_loop);
        }

        // Getting stop_tx clone for this client context
        let stop_rx = self.timer.get_outer_timer_reciever();

        // TODO: MAJOR! I need to reinvent how client context works to do not let loops

        match stop_rx {
            Some(mut stop_tx) => {
                tokio::select! {
                    _ = basic_loop => {},
                    _ = stop_tx.changed() => {

                        let signal = stop_tx.borrow().clone();
                        match signal {
                            SignalType::Work => {},
                            SignalType::Reconnect => {
                                // Loop! You call reconnect but never comeback!
                                self.reconnect().await;
                            },
                            SignalType::Disconnect => {},
                            SignalType::Cancel => {},
                        }
                    }
                }
            }
            None => {
                basic_loop.await;
            }
        }

        // Hm, maybe some last second actions?
        self.on_shutdown().await;

        Ok(())
    }

    // TODO: Create reconnect-dissconnect feature
    async fn disconnect(&self) {}

    async fn reconnect(&self) {}

    pub async fn on_connect(&self) {}
    pub async fn on_shutdown(&self) {}
}
