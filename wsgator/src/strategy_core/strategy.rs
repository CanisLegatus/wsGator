use crate::CommonConfig;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use futures::future;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver as MpscReceiver;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Error as WsError;
use tokio_tungstenite::tungstenite::Message;

// Enumiration for TypeChecking while getting user input from CLI
#[derive(clap::ValueEnum, Clone, Copy)]
pub enum AttackStrategyType {
    Flat,
    RampUp,
    Flood,
}

#[async_trait]
pub trait AttackStrategy: Send + Sync {
    fn get_common_config(&self) -> Arc<CommonConfig>;
    fn handle_special_events(&self) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        Box::pin(future::pending())
    }

    async fn get_writer(
        self: Arc<Self>,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut writer_rx: MpscReceiver<Message>,
        mut stop_rx: WatchReceiver<bool>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            loop {
                tokio::select! {
                    opt_message = writer_rx.recv() => {
                        match opt_message {
                            Some(message) => { sink.send(message).await; },
                            None => { break; }
                        }
                    },
                    _= stop_rx.changed() => { break; }
                }
            }
        })
    }

    fn get_task(
        self: Arc<Self>,
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        mut stop_rx: WatchReceiver<bool>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
    where
        Self: 'static,
    {
        Box::pin(async move {
            let (sink, mut stream) = ws.split();
            let (writer_tx, writer_rx) = mpsc::channel::<Message>(32);
            let writer = self.clone().get_writer(sink, writer_rx, stop_rx.clone());

            tokio::spawn(writer);

            loop {
                tokio::select! {
                    _ = self.handle_base_events(&mut stream, writer_tx.clone() ,i) => {}
                    _ = self.handle_special_events() => {}
                    _ = stop_rx.changed() => {
                        println!("Connection {}: Reached its target time. Sending Close Frame", i);
                        let _ = writer_tx.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
        })
    }

    async fn handle_base_events(
        &self,
        stream: &mut SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        writer_tx: MpscSender<Message>,
        i: u32,
    ) -> Result<bool, WsError> {
        if let Some(msg) = stream.next().await {
            let (proceed, message) = self.handle_messages(msg, i);

            if let Some(message) = message {
                writer_tx.send(message).await;
            }

            if !proceed {
                return Ok(false);
            } else {
                return Ok(true);
            }
        } else {
            return Ok(true);
        }
    }

    fn handle_messages(
        &self,
        msg: Result<Message, tokio_tungstenite::tungstenite::Error>,
        connection_number: u32,
    ) -> (bool, Option<Message>) {
        match msg {
            Ok(message) => match message {
                Message::Ping(bytes) => (true, Some(Message::Pong(bytes))),

                Message::Text(txt) => {
                    println!("Connection {}: Recieved a text: {}", connection_number, txt);
                    (true, None)
                }
                Message::Close(_) => (false, None),

                _ => (false, None),
            },
            Err(e) => {
                println!(
                    "Connection {}: Recieved Err inside a message on listening to next message: {}",
                    connection_number, e
                );
                (false, None)
            }
        }
    }
}
