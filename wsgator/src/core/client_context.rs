use crate::core::behaviour::Behaviour;
use crate::core::monitor::Monitor;
use crate::Arc;
use tokio::sync::mpsc::Receiver as MpscReceiver;

pub struct ClientContext {
    url: String,
    id: u32,
    reciever: MpscReceiver<bool>,
    behaviour: Arc<dyn Behaviour>,
    monitor: Arc<Monitor>,
}
