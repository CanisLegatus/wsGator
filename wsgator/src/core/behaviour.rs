use crate::Arc;
use crate::core::error::WsGatorError;
use crate::core::timer::TimerType;
use async_trait::async_trait;
use futures::SinkExt;
use futures::StreamExt;
use futures::stream::SplitSink;
use futures::stream::SplitStream;
use std::future;
use std::pin::Pin;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Receiver as MpscReceiver;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub type Writer = Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>>;

#[async_trait]
pub trait Behaviour: Send + Sync {
    fn on_message(&self, message: Message) -> Option<Message> {
        match message {
            Message::Ping(bytes) => Some(Message::Pong(bytes)),
            Message::Text(text) => {
                Some(Message::Text(format!("Server sent me text! {text}").into()))
            }
            _ => None,
        }
    }

    fn get_timer(&self) -> TimerType {
        TimerType::Outer
    }

    // Starting
    fn get_writer(
        &self,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut message_rx: MpscReceiver<Message>,
    ) -> Option<Writer> {
        Some(Box::pin(async move {
            while let Some(message) = message_rx.recv().await {
                sink.send(message).await?;
            }

            Ok::<(), WsGatorError>(())
        }))
    }

    async fn on_connect(&self, id: u32, message_tx: &MpscSender<Message>) {
        let _ = message_tx
            .send(Message::Text(format!("Task {} started!", id).into()))
            .await;
    }
    async fn on_error(&self) {}
    async fn on_stop(&self) {}

    // This loop is to define a special logic and it is not in ordinary
    fn get_special_loop(self: Arc<Self>) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }

    // Basic loop to iterate for messages and recieve stop_signal
    async fn get_basic_loop(
        self: Arc<Self>,
        mut stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        message_tx: MpscSender<Message>,
    ) {
        loop {
            match stream.next().await {
                Some(Ok(message)) => {
                    if let Some(message_to_send) = self.on_message(message) {
                        let _ = message_tx.send(message_to_send).await;
                    }
                }
                Some(Err(e)) => {
                    // TODO - log it!
                    println!("ERROR: {e}");
                    break;
                }
                None => {
                    // TODO - log it?
                    println!("None!");
                    break;
                }
            }
        }
    }
}

// TODO:
// Bad Client / Fuzzer - bad data / random json, binary trash
// Lazy Reader / Slow Consumer    / Slow response
// Stateful Scenario              / State scenario - auth/...
// Reconnect Storm

// Completely basic behaviour with all defaults
pub struct DefaultBehaviour {}

/// Structs
/// SilentBehaviour is not responding to anything
pub struct SilentBehaviour {}

/// PingPongBehaviour is anwsering on basic pings and send pings
/// TODO: Unimplemented
pub struct PingPongBehaviour {}

/// FloodBehaviour is sending flood messages to server constantly
/// TODO: Unimplemented
pub struct FloodBehaviour {}

// Implementations
#[async_trait]
impl Behaviour for DefaultBehaviour {}

#[async_trait]
impl Behaviour for SilentBehaviour {
    fn get_writer(
        &self,
        _: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        _: MpscReceiver<Message>,
    ) -> Option<Writer> {
        None
    }

    async fn get_basic_loop(
        self: Arc<Self>,
        _: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        _: MpscSender<Message>,
    ) {
        future::pending::<()>().await;
    }

}

#[async_trait]
impl Behaviour for PingPongBehaviour {}

#[async_trait]
impl Behaviour for FloodBehaviour {}
