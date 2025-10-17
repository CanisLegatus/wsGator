use crate::Arc;
use crate::core::behaviour::Behaviour;
use crate::core::error::WsGatorError;
use crate::core::monitor::Monitor;
use crate::core::timer::Signal;
use crate::core::timer::SignalType;
use futures::StreamExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

// Client Context
// One connection (connection - recieve of message)
// Usage of behaviour to react on events
// Sending metrics to Monitor
// Collecting Errors

// Client Context work
// - Client Context created (idle)
// - Client context run (creates connection, keeps it running, can send message)
// - Client context can receive - disconnect to send end frame and disconnect
// - Client context can receive - reconnect to create a new connection
// - Client context can receive - shutdown to drop everything (why do I need it? Isn't it just disconnect?)

pub struct ClientContext {
    id: u32,
    url: String,
    signal: Arc<Signal>,
    behaviour: Arc<dyn Behaviour>,
    monitor: Arc<Monitor>,
    current_writer: Option<JoinHandle<Result<(), WsGatorError>>>,
    current_special_loop: Option<JoinHandle<()>>,
    current_connection: Option<JoinHandle<()>>,
}

impl ClientContext {
    pub fn new(
        id: u32,
        url: String,
        signal: Arc<Signal>,
        behaviour: Arc<dyn Behaviour>,
        monitor: Arc<Monitor>,
    ) -> Self {
        Self {
            id,
            url,
            signal,
            behaviour,
            monitor,
            current_writer: None,
            current_special_loop: None,
            current_connection: None,
        }
    }

    pub fn get_connection_handle(&mut self) -> Option<JoinHandle<()>> {
        self.current_connection.take()
    }

    async fn get_ws_connection(
        &self,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, WsError> {
        let (ws, _) = connect_async(self.url.clone()).await?;
        Ok(ws)
    }

    pub async fn connect(&mut self) -> Result<(), WsError> {
        // Starting our websocket
        let websocket = self.get_ws_connection().await?;

        // Splitting websocket
        let (sink, stream) = websocket.split();

        // Starting message channel for writer
        let (message_tx, message_rx) = mpsc::channel::<Message>(128);

        // Get writer
        let writer = self.behaviour.get_writer(sink, message_rx);

        // Get special behaviour loop
        let special_loop = self.behaviour.clone().get_special_loop();

        // Get basic behaviour loop (recieveving messages and signals)
        let basic_loop = self
            .behaviour
            .clone()
            .get_basic_loop(stream, message_tx.clone());

        // Starting writer and saving it
        if let Some(writer) = writer {
            self.current_writer = Some(tokio::spawn(writer));
        }

        // Spawning special loop is available
        if let Some(special_loop) = special_loop {
            self.current_special_loop = Some(tokio::spawn(special_loop));
        }

        // Getting signal_tx clone for this client context
        let signal_rx = self.signal.get_outer_signal_reciever();

        self.current_connection = Some(tokio::spawn(async move {
            match signal_rx {
                // If there is external signal
                Some(mut signal_rx) => {
                    tokio::select! {
                        _ = basic_loop => {},
                        _ = signal_rx.changed() => {

                            let signal = signal_rx.borrow().clone();
                            match signal {
                                SignalType::Disconnect => {
                                    _ = message_tx.send(Message::Close(None)).await;
                                },
                            }
                        }
                    };
                }
                None => {
                    basic_loop.await;
                }
            };
        }));

        Ok(())
    }

    async fn disconnect(&self) {}
}
