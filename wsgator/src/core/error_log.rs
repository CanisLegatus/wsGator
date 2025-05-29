use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio_tungstenite::tungstenite::error::Error;

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
        })
    }
    pub fn count(&self, error: Error) {
        match error {
            Error::ConnectionClosed => {
                self.connection_closed.fetch_add(1, Ordering::Relaxed);
            }
            Error::AlreadyClosed => {
                self.already_closed.fetch_add(1, Ordering::Relaxed);
            }
            Error::Io(_) => {
                self.io.fetch_add(1, Ordering::Relaxed);
            }
            Error::Tls(_) => {
                self.tls.fetch_add(1, Ordering::Relaxed);
            }
            Error::Capacity(_) => {
                self.capacity.fetch_add(1, Ordering::Relaxed);
            }
            Error::Protocol(_) => {
                self.protocol.fetch_add(1, Ordering::Relaxed);
            }
            Error::WriteBufferFull(_) => {
                self.write_buffer_full.fetch_add(1, Ordering::Relaxed);
            }
            Error::Utf8 => {
                self.utf8.fetch_add(1, Ordering::Relaxed);
            }
            Error::AttackAttempt => {
                self.attack_attempt.fetch_add(1, Ordering::Relaxed);
            }
            Error::Url(_) => {
                self.url.fetch_add(1, Ordering::Relaxed);
            }
            Error::Http(_) => {
                self.http.fetch_add(1, Ordering::Relaxed);
            }
            Error::HttpFormat(_) => {
                self.http_format.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
