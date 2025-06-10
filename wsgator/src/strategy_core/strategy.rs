use crate::CommonConfig;
use crate::core::error::WsGatorError;
use async_trait::async_trait;
use futures::pin_mut;
use futures::FutureExt;
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
    fn prepare_special_events(
        self: Arc<Self>,
        _: MpscSender<Message>,
        ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send>>{
        Box::pin(future::pending())
    }

    fn get_writer(
        self: Arc<Self>,
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
        ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
        mut stop_rx: WatchReceiver<bool>,
        i: u32,
    ) -> Pin<Box<dyn Future<Output = Result<(), WsGatorError>> + Send + 'static>>
    where
        Self: 'static,
    {
        Box::pin(async move {
            let (sink, mut stream) = ws.split();
            let (writer_tx, writer_rx) = mpsc::channel::<Message>(128);
            let writer = self.clone().get_writer(sink, writer_rx);

            let writer_handler = tokio::spawn(writer);
            let special_event_handle = tokio::spawn(self.clone().prepare_special_events(writer_tx.clone()));

            loop {
                tokio::select! {
                    result = self.run_base_events(&mut stream, &writer_tx ,i) => {
                        match result {
                            Ok(false) => break,
                            Ok(true) => {},
                            Err(e) => { 
                                special_event_handle.abort();
                                writer_handler.abort();
                                return Err(e); }
                        }
                    }
                    //result = &mut special_event => { result?; }
                    result = stop_rx.changed() => {
                        result.map_err(|e| WsGatorError::WatchChannel(e.into()))?;
                        println!("Connection {}: Reached its target time. Sending Close Frame", i);
                        writer_tx.send(Message::Close(None)).await.map_err(|e| {
                            WsGatorError::MpscChannel(e.into())})?;
                        drop(writer_tx);
                        break;
                    }
                }
            }
            
            special_event_handle.abort();    
            writer_handler.await??;

            Ok(())
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
                    "Connection {}: Recieved Err inside a message on listening to next message: {}",
                    connection_number, e
                );
                Err(Box::new(e))
            }
        }
    }
}
