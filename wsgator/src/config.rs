use clap::{Args, Parser, Subcommand};
use std::{path::PathBuf, time::Duration};
use tokio_tungstenite::tungstenite::{
    client::IntoClientRequest,
    http::{HeaderName, HeaderValue},
    protocol::WebSocketConfig,
};

/// WebSocket load testing: hold connections, measure heartbeat, or send paced traffic.
#[derive(Debug, Parser)]
#[command(
    version,
    about,
    after_help = "Examples:\n  wsgator silent --url ws://127.0.0.1:9001 -c 100 -d 30s\n  wsgator ping-pong --interval 1s --timeout 3s\n  wsgator flood --rate 20 --message hello\n\nDuration covers connection setup and traffic. Flood rate is per connection."
)]
pub struct Config {
    #[command(subcommand)]
    pub behavior: Behavior,
    /// Target WebSocket URL (ws:// or wss://)
    #[arg(short, long, global = true, default_value = "ws://127.0.0.1:9001", value_parser = parse_url)]
    pub url: String,
    /// Number of clients; handshakes run concurrently up to --connect-concurrency
    #[arg(short = 'c', long, alias = "connection-number", global = true, default_value = "100", value_parser = clap::value_parser!(u32).range(1..=1_000_000))]
    pub connections: u32,
    /// Total run duration, including connection setup (e.g. 30s, 2m)
    #[arg(short = 'd', long, global = true, default_value = "30s", value_parser = parse_duration)]
    pub duration: Duration,
    /// Maximum simultaneous handshakes
    #[arg(long, global = true, default_value = "256", value_parser = clap::value_parser!(u32).range(1..=65_536))]
    pub connect_concurrency: u32,
    /// Timeout for each DNS/TCP/TLS/WebSocket handshake
    #[arg(long, global = true, default_value = "10s", value_parser = parse_duration)]
    pub connect_timeout: Duration,
    /// Timeout for an individual write or flush
    #[arg(long, global = true, default_value = "5s", value_parser = parse_duration)]
    pub write_timeout: Duration,
    /// Maximum time to complete each close handshake
    #[arg(long, global = true, default_value = "2s", value_parser = parse_duration)]
    pub close_timeout: Duration,
    /// HTTP header, repeatable: -H 'Authorization: Bearer ...'
    #[arg(short = 'H', long = "header", global = true, value_parser = parse_header)]
    pub headers: Vec<(HeaderName, HeaderValue)>,
    /// Offered WebSocket subprotocol (server must select it)
    #[arg(long, global = true, value_parser = parse_protocol)]
    pub subprotocol: Option<String>,
    /// PEM CA certificates to trust in addition to built-in public roots
    #[arg(long, global = true)]
    pub ca_cert: Option<PathBuf>,
    /// Maximum incoming message size in bytes
    #[arg(long, global = true, default_value = "1048576", value_parser = clap::value_parser!(u32).range(1..=67_108_864))]
    pub max_message_bytes: u32,
    /// Write the final report as JSON (use - for stdout)
    #[arg(long, global = true)]
    pub json: Option<PathBuf>,
    /// Print periodic progress to stderr, including when redirected
    #[arg(long, global = true)]
    pub progress: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Behavior {
    /// Hold connections and read messages; automatically answer WebSocket Ping frames
    Silent,
    /// Send WebSocket Ping frames and measure matching Pong round trips
    PingPong {
        #[arg(long, default_value = "1s", value_parser = parse_duration)]
        interval: Duration,
        /// Maximum wait for a matching Pong; at most one Ping is in flight
        #[arg(long, default_value = "5s", value_parser = parse_duration)]
        timeout: Duration,
    },
    /// Send paced text or binary messages; read and count incoming traffic
    Flood(Flood),
}

#[derive(Debug, Clone, Args)]
pub struct Flood {
    /// Messages per second per connected client (no catch-up bursts)
    #[arg(long, default_value = "10", value_parser = clap::value_parser!(u32).range(1..=1_000_000))]
    pub rate: u32,
    /// UTF-8 message body
    #[arg(long, default_value = "wsGator", conflicts_with = "payload_file")]
    pub message: String,
    /// Read the message body once from a file (maximum 16 MiB)
    #[arg(long, conflicts_with = "message")]
    pub payload_file: Option<PathBuf>,
    /// Send binary frames instead of text
    #[arg(long)]
    pub binary: bool,
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    let d = humantime::parse_duration(s).map_err(|e| e.to_string())?;
    if d < Duration::from_millis(1) || d > Duration::from_secs(7 * 24 * 3600) {
        return Err("duration must be between 1ms and 7days".into());
    }
    Ok(d)
}
fn parse_url(s: &str) -> Result<String, String> {
    let u = url::Url::parse(s).map_err(|_| "invalid URL")?;
    if !matches!(u.scheme(), "ws" | "wss")
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
    {
        return Err("expected ws:// or wss:// URL without credentials or fragment; use --header for authentication".into());
    }
    Ok(u.to_string())
}
fn parse_protocol(s: &str) -> Result<String, String> {
    if s.is_empty()
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
    {
        return Err("expected a single HTTP token".into());
    }
    Ok(s.into())
}
fn parse_header(s: &str) -> Result<(HeaderName, HeaderValue), String> {
    let (name, value) = s.split_once(':').ok_or("expected 'Name: value'")?;
    let name = HeaderName::from_bytes(name.trim().as_bytes()).map_err(|_| "invalid header name")?;
    if matches!(name.as_str(), "host" | "connection" | "upgrade")
        || name.as_str().starts_with("sec-websocket-")
    {
        return Err("reserved handshake header; use --subprotocol for protocol negotiation".into());
    }
    let value = HeaderValue::from_str(value.trim()).map_err(|_| "invalid header value")?;
    Ok((name, value))
}
impl Config {
    pub fn request(
        &self,
    ) -> Result<tokio_tungstenite::tungstenite::handshake::client::Request, String> {
        let mut request = self
            .url
            .as_str()
            .into_client_request()
            .map_err(|e| e.to_string())?;
        for (name, value) in &self.headers {
            request.headers_mut().append(name, value.clone());
        }
        if let Some(protocol) = &self.subprotocol {
            request.headers_mut().insert(
                "sec-websocket-protocol",
                HeaderValue::from_str(protocol).map_err(|e| e.to_string())?,
            );
        }
        Ok(request)
    }
    pub fn websocket_config(&self) -> WebSocketConfig {
        WebSocketConfig::default()
            .read_buffer_size(16 * 1024)
            .write_buffer_size(16 * 1024)
            .max_message_size(Some(self.max_message_bytes as usize))
            .max_frame_size(Some(self.max_message_bytes as usize))
    }
}
