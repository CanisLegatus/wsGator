use crate::CommonConfig;
use crate::core::error::WsGatorError;
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
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

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
    async fn handle_special_events(&self) -> Result<(), WsGatorError> {
        future::pending::<()>().await;
        Ok(())
    }

    fn get_writer(
        self: Arc<Self>,
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut writer_rx: MpscReceiver<Message>,
        mut stop_rx: WatchReceiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>> {
        Box::pin(async move {
            println!("WRITER: Entering loop");
            loop {
                tokio::select! {
                    opt_message = writer_rx.recv() => {
                        match opt_message {
                            Some(message) => { 
                                eprintln!("WRITER: Before send");
                                sink.send(message).await?;
                                eprintln!("WRITER: After send");
                            },
                            None => { 
                                eprintln!("WRITER: Dropping because of NONE");
                                break; }
                        }
                    },
                    result = stop_rx.changed() => {
                        eprintln!("WRITER: Dropping because of stop_rx.changed()");
                        result.map_err(|e| WsGatorError::WatchChannel(e.into()))?;
                        break; }
                }
            }
            println!("WRITER: Exiting loop");
            Ok(())
        })
    }

    fn get_task(
        self: Arc<Self>,
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        mut stop_rx: WatchReceiver<bool>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send + 'static>>
    where
        Self: 'static,
    {
        Box::pin(async move {
            let (sink, mut stream) = ws.split();
            let (writer_tx, writer_rx) = mpsc::channel::<Message>(32);
            let writer = self.clone().get_writer(sink, writer_rx, stop_rx.clone());

            let writer_handler = tokio::spawn(writer);

            loop {
                tokio::select! {
                    result = self.handle_base_events(&mut stream, writer_tx.clone() ,i) => {
                        match result {
                            Ok(false) => break,
                            Ok(true) => {},
                            Err(e) => { return Err(e); }
                        }
                    }
                    result = self.handle_special_events() => { result?; }
                    result = stop_rx.changed() => {
                        result.map_err(|e| WsGatorError::WatchChannel(e.into()))?;
                        println!("Connection {}: Reached its target time. Sending Close Frame", i);
                        writer_tx.send(Message::Close(None)).await.map_err(|e| WsGatorError::MpscChannel(e.into()))?;
                        break;
                    }
                }
            }
            let _ = writer_handler.await;
            Ok(())
        })
    }

    async fn handle_base_events(
        &self,
        stream: &mut SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
        writer_tx: MpscSender<Message>,
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
                        .map_err(|e| {
                            println!("BASE: Error on MPSC Msg: {:?}, Err: {}", message, e); 
                            WsGatorError::MpscChannel(e.into())})?;
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

                Message::Text(txt) => {
                    println!("Connection {}: Recieved a text: {}", connection_number, txt);
                    Ok((true, None))
                }
                Message::Close(_) => Ok((false, None)),

                _ => Ok((true, None)),
            },
            Err(e) => {
                println!(
                    "Connection {}: Recieved Err inside a message on listening to next message: {}",
                    connection_number, e
                );
                Err(Box::new(e))
            }
        }
    }
}
