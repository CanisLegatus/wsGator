use tokio::sync::watch;
use tokio::sync::watch::{Receiver as WatchReceiver, Sender as WatchSender};

#[derive(Clone)]
pub struct Signal {
    reciever: Option<WatchReceiver<SignalType>>,
    sender: Option<WatchSender<SignalType>>,
    timer_type: TimerType,
}

impl Signal {
    pub fn new(timer_type: TimerType) -> Self {
        match timer_type {
            TimerType::Empty => Self {
                reciever: None,
                sender: None,
                timer_type,
            },
            TimerType::Outer => {
                let (stop_tx, stop_rx) = watch::channel(SignalType::Work);
                Self {
                    reciever: Some(stop_rx),
                    sender: Some(stop_tx),
                    timer_type,
                }
            }
        }
    }

    pub fn get_outer_timer(&mut self) -> Option<WatchSender<SignalType>> {
        self.sender.take()
    }

    pub fn get_outer_timer_reciever(&self) -> Option<WatchReceiver<SignalType>> {
        self.reciever.clone()
    }
}

#[derive(Clone)]
pub enum TimerType {
    Empty,
    Outer,
}

#[derive(Clone)]
pub enum SignalType {
    Work,
    Disconnect,
    Reconnect,
    Cancel,
}
