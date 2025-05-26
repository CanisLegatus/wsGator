use tokio::sync::mpsc::error::{SendError as MpscSendError, TryRecvError, TrySendError, SendTimeoutError};
use tokio::sync::watch::error::{SendError as WatchSendError, RecvError};
use std::{error::Error, fmt::Display};
use tokio_tungstenite::tungstenite::Error as WsError;
use std::fmt::Debug;

#[derive(Debug)]
pub enum WsGatorError<T: Debug> {
    Mpsc(MpscChannelError<T>),
    Watch(WatchChannelError<T>),
    WsError(WsError),
}

impl<T: Debug> Error for WsGatorError<T> {}

impl<T: Debug> Display for WsGatorError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsGatorError::Mpsc(_) => {
                write!(f, "ChannelError")
            },
            WsGatorError::WsError(_) => {
                write!(f, "WsError")
            },
            _ => {
                write!(f, "Other Error")
            }
        }
    }
}

impl<T: Debug> From<MpscChannelError<T>> for WsGatorError<T> {
    fn from(value: MpscChannelError<T>) -> Self {
       match value {
            MpscChannelError::Send(inner) => {
                WsGatorError::Mpsc(MpscChannelError::Send(inner))
            },
            MpscChannelError::TrySend(inner) => {
                WsGatorError::Mpsc(MpscChannelError::TrySend(inner))    
            },
            MpscChannelError::SendTimeout(inner) => {
                WsGatorError::Mpsc(MpscChannelError::SendTimeout(inner))
            },
            MpscChannelError::TryRecv(inner) => {
                WsGatorError::Mpsc(MpscChannelError::TryRecv(inner))
            }
        } 
    }
}

#[derive(Debug)]
pub enum MpscChannelError<T> {
    Send(MpscSendError<T>),
    TrySend(TrySendError<T>),
    SendTimeout(SendTimeoutError<T>),
    TryRecv(TryRecvError),
}

#[derive(Debug)]
pub enum WatchChannelError<T> {
    Send(WatchSendError<T>),
    Recv(RecvError),
}
