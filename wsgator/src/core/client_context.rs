use crate::core::behaviour::Behaviour;
use crate::core::monitor::Monitor;
use crate::Arc;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::watch::Receiver as WatchReceiver;
use tokio_tungstenite::tungstenite::Message;

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
    pub fn new(url: String, id: u32, writer_sender: MpscSender<Message>, stop_reciever: WatchReceiver<bool>, behaviour: Arc<dyn Behaviour>, monitor: Arc<Monitor>) -> Self {
        Self {
            url,
            id,
            writer_sender,
            stop_reciever,
            behaviour,
            monitor,
        }
    }
    
    pub async fn run(&self) {}
    pub async fn send_message(&self) {}
    pub async fn shutdown(&self) {}


}
