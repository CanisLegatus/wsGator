use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio_tungstenite::tungstenite::error::Error as WsError;

use super::error::{MpscChannelError, WatchChannelError, WsGatorError};

#[derive(Debug)]
pub struct ErrorLog {
    connection_closed: AtomicU32,
    already_closed: AtomicU32,
    io: AtomicU32,
    tls: AtomicU32,
    capacity: AtomicU32,
    protocol: AtomicU32,
    write_buffer_full: AtomicU32,
    utf8: AtomicU32,
    attack_attempt: AtomicU32,
    url: AtomicU32,
    http: AtomicU32,
    http_format: AtomicU32,

    mpsc_send: AtomicU32,
    mpsc_try_send: AtomicU32,
    mpsc_send_timeout: AtomicU32,
    mpsc_try_recv: AtomicU32,

    watcher_send: AtomicU32,
    watcher_recv: AtomicU32,

    join_err: AtomicU32,
}

impl ErrorLog {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            connection_closed: AtomicU32::new(0),
            already_closed: AtomicU32::new(0),
            io: AtomicU32::new(0),
            tls: AtomicU32::new(0),
            capacity: AtomicU32::new(0),
            protocol: AtomicU32::new(0),
            write_buffer_full: AtomicU32::new(0),
            utf8: AtomicU32::new(0),
            attack_attempt: AtomicU32::new(0),
            url: AtomicU32::new(0),
            http: AtomicU32::new(0),
            http_format: AtomicU32::new(0),

            mpsc_send: AtomicU32::new(0),
            mpsc_try_send: AtomicU32::new(0),
            mpsc_send_timeout: AtomicU32::new(0),
            mpsc_try_recv: AtomicU32::new(0),

            watcher_send: AtomicU32::new(0),
            watcher_recv: AtomicU32::new(0),

            join_err: AtomicU32::new(0),
        })
    }

    fn count_ws_err(&self, error: WsError) {
        match error {
            WsError::ConnectionClosed => {
                self.connection_closed.fetch_add(1, Ordering::Relaxed);
            }
            WsError::AlreadyClosed => {
                self.already_closed.fetch_add(1, Ordering::Relaxed);
            }
            WsError::Io(_) => {
                self.io.fetch_add(1, Ordering::Relaxed);
            }
            WsError::Tls(_) => {
                self.tls.fetch_add(1, Ordering::Relaxed);
            }
            WsError::Capacity(_) => {
                self.capacity.fetch_add(1, Ordering::Relaxed);
            }
            WsError::Protocol(inner) => {
                println!("Met PROTOCOL ERR: {}", inner);
                self.protocol.fetch_add(1, Ordering::Relaxed);
            }
            WsError::WriteBufferFull(_) => {
                self.write_buffer_full.fetch_add(1, Ordering::Relaxed);
            }
            WsError::Utf8 => {
                self.utf8.fetch_add(1, Ordering::Relaxed);
            }
            WsError::AttackAttempt => {
                self.attack_attempt.fetch_add(1, Ordering::Relaxed);
            }
            WsError::Url(_) => {
                self.url.fetch_add(1, Ordering::Relaxed);
            }
            WsError::Http(_) => {
                self.http.fetch_add(1, Ordering::Relaxed);
            }
            WsError::HttpFormat(_) => {
                self.http_format.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn count_mpsc_err(&self, error: MpscChannelError) {
        match error {
            MpscChannelError::Send(_) => {
                self.mpsc_send.fetch_add(1, Ordering::Relaxed);
            }
            MpscChannelError::SendTimeout(_) => {
                self.mpsc_send_timeout.fetch_add(1, Ordering::Relaxed);
            }
            MpscChannelError::TryRecv(_) => {
                self.mpsc_try_recv.fetch_add(1, Ordering::Relaxed);
            }
            MpscChannelError::TrySend(_) => {
                self.mpsc_try_send.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn count_watch_err(&self, error: WatchChannelError) {
        match error {
            WatchChannelError::Send(_) => {
                self.watcher_send.fetch_add(1, Ordering::Relaxed);
            }
            WatchChannelError::Recv(_) => {
                self.watcher_recv.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn count(&self, error: WsGatorError) {
        match error {
            WsGatorError::WsError(inner) => {
                self.count_ws_err(*inner);
            }

            WsGatorError::MpscChannel(inner) => {
                self.count_mpsc_err(inner);
            }

            WsGatorError::WatchChannel(inner) => {
                self.count_watch_err(inner);
            }

            WsGatorError::JoinError(_) => {
                self.join_err.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
