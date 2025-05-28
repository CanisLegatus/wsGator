use std::fmt::Debug;
use std::{error::Error, fmt::Display};
use tokio::sync::mpsc::error::{
    SendError as MpscSendError, SendTimeoutError, TryRecvError, TrySendError,
};
use tokio::sync::watch::error::{RecvError, SendError as WatchSendError};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

#[derive(Debug)]
pub enum WsGatorError {
    MpscChannel(MpscChannelError),
    WatchChannel(WatchChannelError),
    WsError(WsError),
}

impl Error for WsGatorError {}

impl Display for WsGatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsGatorError::MpscChannel(inner) => write!(f, "MPSC Channel error: {}", inner),
            WsGatorError::WatchChannel(inner) => write!(f, "Watch Channel error: {}", inner),
            WsGatorError::WsError(inner) => write!(f, "Web Socket Error: {}", inner),
        }
    }
}

impl From<WsError> for WsGatorError {
    fn from(value: WsError) -> Self {
        WsGatorError::WsError(value)
    }
}

impl From<MpscChannelError> for WsGatorError {
    fn from(value: MpscChannelError) -> Self {
        WsGatorError::MpscChannel(value)
    }
}

impl From<WatchChannelError> for WsGatorError {
    fn from(value: WatchChannelError) -> Self {
        WsGatorError::WatchChannel(value)
    }
}

#[derive(Debug)]
pub enum MpscChannelError {
    Send(MpscSendError<Message>),
    TrySend(TrySendError<Message>),
    SendTimeout(SendTimeoutError<Message>),
    TryRecv(TryRecvError),
}

impl From<MpscSendError<Message>> for MpscChannelError {
    fn from(value: MpscSendError<Message>) -> Self {
        MpscChannelError::Send(value)
    }
}

impl From<TrySendError<Message>> for MpscChannelError {
    fn from(value: TrySendError<Message>) -> Self {
        MpscChannelError::TrySend(value)
    }
}

impl From<SendTimeoutError<Message>> for MpscChannelError {
    fn from(value: SendTimeoutError<Message>) -> Self {
        MpscChannelError::SendTimeout(value)
    }
}

impl From<TryRecvError> for MpscChannelError {
    fn from(value: TryRecvError) -> Self {
        MpscChannelError::TryRecv(value)
    }
}


impl Display for MpscChannelError {
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
pub enum WatchChannelError {
    Send(WatchSendError<bool>),
    Recv(RecvError),
}

impl From<WatchSendError<bool>> for WatchChannelError {
    fn from(value: WatchSendError<bool>) -> Self {
        WatchChannelError::Send(value)
    }
}

impl From<RecvError> for WatchChannelError {
    fn from(value: RecvError) -> Self {
        WatchChannelError::Recv(value)
    }
}

impl Display for WatchChannelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchChannelError::Send(inner) => write!(f, "Send error: {:?}", inner),
            WatchChannelError::Recv(inner) => write!(f, "Receive error: {:?}", inner),
        }
    }
}
