use tokio::sync::watch;
use tokio::sync::watch::{Receiver as WatchReceiver, Sender as WatchSender};

#[derive(Clone)]
pub struct Timer {
    reciever: Option<WatchReceiver<bool>>,
    sender: Option<WatchSender<bool>>,
    timer_type: TimerType,
}

impl Timer {
    pub fn new(timer_type: TimerType) -> Self {
        match timer_type {
            TimerType::Empty => Self {
                reciever: None,
                sender: None,
                timer_type,
            },
            TimerType::Outer => {
                let (stop_tx, stop_rx) = watch::channel(false);
                Self {
                    reciever: Some(stop_rx),
                    sender: Some(stop_tx),
                    timer_type,
                }
            }
        }
    }

    pub fn get_outer_timer(&mut self) -> Option<WatchSender<bool>> {
        self.sender.take()
    }
}

#[derive(Clone)]
pub enum TimerType {
    Empty,
    Outer,
}
