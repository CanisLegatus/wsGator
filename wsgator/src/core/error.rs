use std::fmt::Debug;
use std::{error::Error, fmt::Display};
use tokio::sync::mpsc::error::{
    SendError as MpscSendError, SendTimeoutError, TryRecvError, TrySendError,
};
use tokio::sync::watch::error::{RecvError, SendError as WatchSendError};
use tokio_tungstenite::tungstenite::Error as WsError;

#[derive(Debug)]
pub enum WsGatorError<T: Debug> {
    MpscChannel(MpscChannelError<T>),
    WatchChannel(WatchChannelError<T>),
    WsError(WsError),
}

impl<T: Debug> Error for WsGatorError<T> {}

impl<T: Debug> Display for WsGatorError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsGatorError::MpscChannel(inner) => write!(f, "MPSC Channel error: {}", inner),
            WsGatorError::WatchChannel(inner) => write!(f, "Watch Channel error: {}", inner),
            WsGatorError::WsError(inner) => write!(f, "Web Socket Error: {}", inner),
        }
    }
}

impl<T: Debug> From<MpscChannelError<T>> for WsGatorError<T> {
    fn from(value: MpscChannelError<T>) -> Self {
        WsGatorError::MpscChannel(value)
    }
}

impl<T: Debug> From<WatchChannelError<T>> for WsGatorError<T> {
    fn from(value: WatchChannelError<T>) -> Self {
        WsGatorError::WatchChannel(value)
    }
}

#[derive(Debug)]
pub enum MpscChannelError<T> {
    Send(MpscSendError<T>),
    TrySend(TrySendError<T>),
    SendTimeout(SendTimeoutError<T>),
    TryRecv(TryRecvError),
}

impl<T: Debug> Display for MpscChannelError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MpscChannelError::Send(inner) => write!(f, "Send error: {:?}", inner),
            MpscChannelError::TrySend(inner) => write!(f, "Try Send error: {:?}", inner),
            MpscChannelError::SendTimeout(inner) => write!(f, "Send Timeout error: {:?}", inner),
            MpscChannelError::TryRecv(inner) => write!(f, "Try Recieve error: {:?}", inner),
        }
    }
}

#[derive(Debug)]
pub enum WatchChannelError<T> {
    Send(WatchSendError<T>),
    Recv(RecvError),
}

impl<T: Debug> Display for WatchChannelError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchChannelError::Send(inner) => write!(f, "Send error: {:?}", inner),
            WatchChannelError::Recv(inner) => write!(f, "Receive error: {:?}", inner),
        }
    }
}
