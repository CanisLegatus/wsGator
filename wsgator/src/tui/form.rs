use crate::config::Config;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Silent,
    #[default]
    PingPong,
    Flood,
}
impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Silent => "SILENT",
            Self::PingPong => "PING / PONG",
            Self::Flood => "FLOOD",
        }
    }
    pub fn command(self) -> &'static str {
        match self {
            Self::Silent => "silent",
            Self::PingPong => "ping-pong",
            Self::Flood => "flood",
        }
    }
    pub fn next(self, backward: bool) -> Self {
        match (self, backward) {
            (Self::Silent, false) | (Self::Flood, true) => Self::PingPong,
            (Self::PingPong, false) | (Self::Silent, true) => Self::Flood,
            _ => Self::Silent,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Draft {
    pub mode: Mode,
    pub url: String,
    pub connections: String,
    pub duration: String,
    pub interval: String,
    pub pong_timeout: String,
    pub rate: String,
    #[serde(skip, default = "default_message")]
    pub message: String,
    pub binary: bool,
    pub payload_file: String,
    pub concurrency: String,
    pub connect_timeout: String,
    pub write_timeout: String,
    pub close_timeout: String,
    pub max_message: String,
    pub subprotocol: String,
    pub ca_cert: String,
    #[serde(skip)]
    pub headers: String,
}
fn default_message() -> String {
    "wsGator".into()
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            mode: Mode::PingPong,
            url: "ws://127.0.0.1:9001".into(),
            connections: "100".into(),
            duration: "30s".into(),
            interval: "1s".into(),
            pong_timeout: "5s".into(),
            rate: "10".into(),
            message: default_message(),
            binary: false,
            payload_file: String::new(),
            concurrency: "256".into(),
            connect_timeout: "10s".into(),
            write_timeout: "5s".into(),
            close_timeout: "2s".into(),
            max_message: "1048576".into(),
            subprotocol: String::new(),
            ca_cert: String::new(),
            headers: String::new(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Url,
    Connections,
    Duration,
    Interval,
    PongTimeout,
    Rate,
    Message,
    PayloadFile,
    Binary,
    Concurrency,
    ConnectTimeout,
    WriteTimeout,
    CloseTimeout,
    MaxMessage,
    Subprotocol,
    CaCert,
    Headers,
}
impl Field {
    pub fn label(self) -> &'static str {
        match self {
            Self::Url => "TARGET ENDPOINT",
            Self::Connections => "CONNECTIONS",
            Self::Duration => "OBSERVATION WINDOW",
            Self::Interval => "PING INTERVAL",
            Self::PongTimeout => "PONG TIMEOUT",
            Self::Rate => "MESSAGES / SEC / CLIENT",
            Self::Message => "MESSAGE BODY",
            Self::PayloadFile => "PAYLOAD FILE (OPTIONAL)",
            Self::Binary => "FRAME TYPE",
            Self::Concurrency => "CONCURRENT HANDSHAKES",
            Self::ConnectTimeout => "CONNECT TIMEOUT",
            Self::WriteTimeout => "WRITE TIMEOUT",
            Self::CloseTimeout => "CLOSE TIMEOUT",
            Self::MaxMessage => "MAX INCOMING BYTES",
            Self::Subprotocol => "SUBPROTOCOL (OPTIONAL)",
            Self::CaCert => "CA CERTIFICATE (OPTIONAL)",
            Self::Headers => "HTTP HEADERS (SESSION ONLY)",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Self::Url => "ws:// or verified wss://. Default: local echo server on port 9001.",
            Self::Connections => "Each client connects once. Failed clients are not replaced.",
            Self::Duration => "Includes connection setup. Examples: 500ms, 30s, 2m.",
            Self::Interval => "Delay between Ping frames. At most one Ping is in flight.",
            Self::PongTimeout => "Fail a client if its matching Pong does not arrive in time.",
            Self::Rate => "Per-client target; backpressure skips slots without catch-up bursts.",
            Self::Message => {
                "UTF-8 body shared by clients. Session only; excluded from saved profiles."
            }
            Self::PayloadFile => {
                "Overrides the message body. File limit: 16 MiB. Text requires UTF-8."
            }
            Self::Binary => "Enter or Space switches between text and binary frames.",
            Self::Concurrency => {
                "Caps simultaneous handshakes, not the final number of active sockets."
            }
            Self::Headers => {
                "Name: value. Separate headers with literal \\n; never saved in profiles."
            }
            Self::CaCert => {
                "Add a PEM CA certificate while keeping certificate verification enabled."
            }
            Self::Subprotocol => "Offer one protocol token. The server must select it.",
            Self::MaxMessage => "Incoming message/frame limit. Default: 1 MiB.",
            _ => "Examples: 250ms, 5s. Duration range: 1ms to 7days.",
        }
    }
}
impl Draft {
    pub fn fields(&self, advanced: bool) -> Vec<Field> {
        use Field::*;
        let mut fields = vec![Url, Connections, Duration];
        match self.mode {
            Mode::Silent => (),
            Mode::PingPong => fields.extend([Interval, PongTimeout]),
            Mode::Flood => fields.extend([Rate, Message, PayloadFile, Binary]),
        }
        if advanced {
            fields.extend([
                Concurrency,
                ConnectTimeout,
                WriteTimeout,
                CloseTimeout,
                MaxMessage,
                Subprotocol,
                CaCert,
                Headers,
            ]);
        }
        fields
    }
    pub fn value(&self, field: Field) -> &str {
        match field {
            Field::Url => &self.url,
            Field::Connections => &self.connections,
            Field::Duration => &self.duration,
            Field::Interval => &self.interval,
            Field::PongTimeout => &self.pong_timeout,
            Field::Rate => &self.rate,
            Field::Message => &self.message,
            Field::PayloadFile => &self.payload_file,
            Field::Binary => {
                if self.binary {
                    "BINARY"
                } else {
                    "TEXT"
                }
            }
            Field::Concurrency => &self.concurrency,
            Field::ConnectTimeout => &self.connect_timeout,
            Field::WriteTimeout => &self.write_timeout,
            Field::CloseTimeout => &self.close_timeout,
            Field::MaxMessage => &self.max_message,
            Field::Subprotocol => &self.subprotocol,
            Field::CaCert => &self.ca_cert,
            Field::Headers => &self.headers,
        }
    }
    pub fn value_mut(&mut self, field: Field) -> Option<&mut String> {
        Some(match field {
            Field::Url => &mut self.url,
            Field::Connections => &mut self.connections,
            Field::Duration => &mut self.duration,
            Field::Interval => &mut self.interval,
            Field::PongTimeout => &mut self.pong_timeout,
            Field::Rate => &mut self.rate,
            Field::Message => &mut self.message,
            Field::PayloadFile => &mut self.payload_file,
            Field::Binary => return None,
            Field::Concurrency => &mut self.concurrency,
            Field::ConnectTimeout => &mut self.connect_timeout,
            Field::WriteTimeout => &mut self.write_timeout,
            Field::CloseTimeout => &mut self.close_timeout,
            Field::MaxMessage => &mut self.max_message,
            Field::Subprotocol => &mut self.subprotocol,
            Field::CaCert => &mut self.ca_cert,
            Field::Headers => &mut self.headers,
        })
    }
    pub fn config(&self) -> Result<Config, String> {
        let mut args = vec!["wsgator".to_string(), self.mode.command().into()];
        for (name, value) in [
            ("--url", &self.url),
            ("--connections", &self.connections),
            ("--duration", &self.duration),
            ("--connect-concurrency", &self.concurrency),
            ("--connect-timeout", &self.connect_timeout),
            ("--write-timeout", &self.write_timeout),
            ("--close-timeout", &self.close_timeout),
            ("--max-message-bytes", &self.max_message),
        ] {
            args.extend([name.into(), value.clone()]);
        }
        match self.mode {
            Mode::Silent => (),
            Mode::PingPong => args.extend([
                "--interval".into(),
                self.interval.clone(),
                "--timeout".into(),
                self.pong_timeout.clone(),
            ]),
            Mode::Flood => {
                args.extend(["--rate".into(), self.rate.clone()]);
                if self.payload_file.is_empty() {
                    args.extend(["--message".into(), self.message.clone()]);
                } else {
                    args.extend(["--payload-file".into(), self.payload_file.clone()]);
                }
                if self.binary {
                    args.push("--binary".into());
                }
            }
        }
        if !self.subprotocol.is_empty() {
            args.extend(["--subprotocol".into(), self.subprotocol.clone()]);
        }
        if !self.ca_cert.is_empty() {
            args.extend(["--ca-cert".into(), self.ca_cert.clone()]);
        }
        for header in self.headers.split("\\n").filter(|s| !s.trim().is_empty()) {
            args.extend(["--header".into(), header.trim().into()]);
        }
        Config::try_parse_from(args).map_err(|error| {
            error
                .to_string()
                .lines()
                .take_while(|line| !line.starts_with("Usage:"))
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string()
        })
    }
    pub fn from_config(c: &Config) -> Self {
        let mut draft = Self {
            url: c.url.clone(),
            connections: c.connections.to_string(),
            duration: humantime::format_duration(c.duration).to_string(),
            concurrency: c.connect_concurrency.to_string(),
            connect_timeout: humantime::format_duration(c.connect_timeout).to_string(),
            write_timeout: humantime::format_duration(c.write_timeout).to_string(),
            close_timeout: humantime::format_duration(c.close_timeout).to_string(),
            max_message: c.max_message_bytes.to_string(),
            subprotocol: c.subprotocol.clone().unwrap_or_default(),
            ca_cert: c
                .ca_cert
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            headers: c
                .headers
                .iter()
                .map(|(name, value)| format!("{name}: {}", value.to_str().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("\\n"),
            ..Self::default()
        };
        match &c.behavior {
            crate::config::Behavior::Silent => draft.mode = Mode::Silent,
            crate::config::Behavior::PingPong { interval, timeout } => {
                draft.mode = Mode::PingPong;
                draft.interval = humantime::format_duration(*interval).to_string();
                draft.pong_timeout = humantime::format_duration(*timeout).to_string();
            }
            crate::config::Behavior::Flood(f) => {
                draft.mode = Mode::Flood;
                draft.rate = f.rate.to_string();
                draft.message = f.message.clone();
                draft.binary = f.binary;
                draft.payload_file = f
                    .payload_file
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
        }
        draft
    }
}

pub struct Profile {
    pub path: Option<PathBuf>,
}
impl Profile {
    pub fn discover(disabled: bool) -> Self {
        Self {
            path: if disabled {
                None
            } else {
                directories::ProjectDirs::from("dev", "wsGator", "wsGator")
                    .map(|p| p.config_dir().join("profile.json"))
            },
        }
    }
    pub fn load(&self) -> Result<Draft, String> {
        let Some(path) = &self.path else {
            return Ok(Draft::default());
        };
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "Saved profile could not be read; using defaults.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Draft::default()),
            Err(_) => Err("Saved profile is unavailable; using defaults.".into()),
        }
    }
    pub fn save(&self, draft: &Draft) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut clean = draft.clone();
        // Authentication query parameters are ephemeral, like HTTP headers and payload text.
        if let Ok(mut url) = url::Url::parse(&clean.url) {
            url.set_query(None);
            clean.url = url.to_string();
        }
        let bytes = serde_json::to_vec_pretty(&clean).map_err(|e| e.to_string())?;
        fs::create_dir_all(path.parent().ok_or("invalid profile path")?)
            .map_err(|e| format!("Cannot create profile directory: {e}"))?;
        fs::write(path, bytes).map_err(|e| format!("Cannot save profile: {e}"))
    }
}
