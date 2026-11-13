use crate::{
    config::{Behavior, Config},
    report::{Counters, Latency, Report, add, inc, load},
};
use futures::{SinkExt, StreamExt};
use std::{
    io::Cursor,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    net::TcpStream,
    sync::Semaphore,
    task::JoinSet,
    time::{Instant, MissedTickBehavior, interval, sleep_until, timeout},
};
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config,
    tungstenite::{Error as WsError, Message},
};
use tokio_util::sync::CancellationToken;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
const MAX_PAYLOAD: usize = 16 * 1024 * 1024;

struct Prepared {
    config: Config,
    payload: Option<Message>,
    connector: Connector,
}
impl Prepared {
    async fn new(config: Config) -> Result<Self, String> {
        config.request()?;
        let payload = if let Behavior::Flood(f) = &config.behavior {
            let data = if let Some(path) = &f.payload_file {
                let file = tokio::fs::File::open(path)
                    .await
                    .map_err(|e| format!("cannot open payload file: {e}"))?;
                let mut data = Vec::new();
                file.take((MAX_PAYLOAD + 1) as u64)
                    .read_to_end(&mut data)
                    .await
                    .map_err(|e| format!("cannot read payload: {e}"))?;
                data
            } else {
                f.message.as_bytes().to_vec()
            };
            if data.len() > MAX_PAYLOAD {
                return Err("payload exceeds 16 MiB".into());
            }
            Some(if f.binary {
                Message::Binary(data.into())
            } else {
                Message::Text(
                    String::from_utf8(data)
                        .map_err(|_| "text payload must be UTF-8; use --binary")?
                        .into(),
                )
            })
        } else {
            None
        };
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        if let Some(path) = &config.ca_cert {
            let data = tokio::fs::read(path)
                .await
                .map_err(|e| format!("cannot read CA file: {e}"))?;
            let certs = rustls_pemfile::certs(&mut Cursor::new(data))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("invalid CA PEM: {e}"))?;
            if certs.is_empty() {
                return Err("CA file contains no certificates".into());
            }
            for cert in certs {
                roots
                    .add(cert)
                    .map_err(|e| format!("invalid CA certificate: {e}"))?;
            }
        }
        let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_root_certificates(roots)
        .with_no_client_auth();
        Ok(Self {
            config,
            payload,
            connector: Connector::Rustls(Arc::new(tls)),
        })
    }
}

/// Run a flat load and drain every client before returning. Cancelling `stop` requests
/// graceful shutdown; dropping the run future aborts its owned client tasks.
pub async fn run(config: Config, stop: CancellationToken) -> Result<Report, String> {
    let prepared = Arc::new(Prepared::new(config).await?);
    let config = &prepared.config;
    let counters = Arc::new(Counters::default());
    let semaphore = Arc::new(Semaphore::new(config.connect_concurrency as usize));
    let shutdown = stop.child_token();
    let started = Instant::now();
    let deadline = started + config.duration;
    let mut tasks = JoinSet::new();
    for id in 0..config.connections {
        tasks.spawn(client(
            id,
            prepared.clone(),
            counters.clone(),
            semaphore.clone(),
            shutdown.clone(),
        ));
    }
    let mut progress = interval(Duration::from_secs(1));
    progress.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Keep the configured observation window even if every connection fails early.
    loop {
        tokio::select! {
            biased;
            _ = stop.cancelled() => break,
            _ = sleep_until(deadline) => break,
            result = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(Err(_)) = result { counters.error("client_task_failed"); }
            }
            _ = progress.tick(), if config.progress => {
                eprintln!("active={} connected={} sent={} received={} elapsed={:.1}s", load(&counters.active), load(&counters.connected), load(&counters.messages_sent), load(&counters.messages_received), started.elapsed().as_secs_f64());
            }
        }
    }
    let load_duration_seconds = started.elapsed().as_secs_f64();
    shutdown.cancel();
    while let Some(result) = tasks.join_next().await {
        if result.is_err() {
            counters.error("client_task_failed");
        }
    }
    if !stop.is_cancelled()
        && (load(&counters.attempted) < config.connections as u64
            || load(&counters.cancelled_connects) > 0)
    {
        counters.error("load_incomplete");
    }
    let mut target = url::Url::parse(&config.url).map_err(|e| e.to_string())?;
    target.set_query(None);
    let (behavior, rate, ping_interval, pong_timeout) = match &config.behavior {
        Behavior::Silent => ("silent", None, None, None),
        Behavior::PingPong { interval, timeout } => (
            "ping-pong",
            None,
            Some(interval.as_secs_f64()),
            Some(timeout.as_secs_f64()),
        ),
        Behavior::Flood(f) => (
            if f.binary {
                "flood-binary"
            } else {
                "flood-text"
            },
            Some(f.rate),
            None,
            None,
        ),
    };
    Ok(Report {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        target: target.to_string(),
        behavior: behavior.into(),
        requested_connections: config.connections,
        connect_concurrency: config.connect_concurrency,
        connect_timeout_seconds: config.connect_timeout.as_secs_f64(),
        write_timeout_seconds: config.write_timeout.as_secs_f64(),
        close_timeout_seconds: config.close_timeout.as_secs_f64(),
        max_message_bytes: config.max_message_bytes,
        subprotocol: config.subprotocol.clone(),
        configured_duration_seconds: config.duration.as_secs_f64(),
        load_duration_seconds,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        interrupted: stop.is_cancelled(),
        payload_bytes: prepared.payload.as_ref().map(Message::len),
        messages_per_second_per_connection: rate,
        ping_interval_seconds: ping_interval,
        pong_timeout_seconds: pong_timeout,
        attempted: load(&counters.attempted),
        connected: load(&counters.connected),
        peak_active: load(&counters.peak_active),
        active_at_end: load(&counters.active),
        completed: load(&counters.completed),
        cancelled_connects: load(&counters.cancelled_connects),
        not_attempted: config.connections as u64 - load(&counters.attempted),
        messages_sent: load(&counters.messages_sent),
        messages_received: load(&counters.messages_received),
        bytes_sent: load(&counters.bytes_sent),
        bytes_received: load(&counters.bytes_received),
        pings_sent: load(&counters.pings_sent),
        pings_received: load(&counters.pings_received),
        pongs_received: load(&counters.pongs_received),
        pongs_sent: load(&counters.pongs_sent),
        pending_pings_at_stop: load(&counters.pending_pings_at_stop),
        missed_send_slots: load(&counters.missed_send_slots),
        send_messages_per_second: load(&counters.messages_sent) as f64
            / load_duration_seconds.max(0.000_001),
        receive_messages_per_second: load(&counters.messages_received) as f64
            / load_duration_seconds.max(0.000_001),
        handshake: Latency::snapshot(&counters.handshake),
        ping_rtt: Latency::snapshot(&counters.rtt),
        errors: counters.errors.lock().unwrap().clone(),
    })
}

struct Active(Arc<Counters>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::Relaxed);
        inc(&self.0.completed);
    }
}
async fn client(
    id: u32,
    p: Arc<Prepared>,
    c: Arc<Counters>,
    semaphore: Arc<Semaphore>,
    stop: CancellationToken,
) {
    let permit = tokio::select! {
        biased;
        _ = stop.cancelled() => return,
        permit = semaphore.acquire() => permit.expect("semaphore stays open"),
    };
    if stop.is_cancelled() {
        return;
    }
    inc(&c.attempted);
    let started = Instant::now();
    let request = p
        .config
        .request()
        .expect("request validated before spawning clients");
    let result = tokio::select! {
        biased;
        _ = stop.cancelled() => { inc(&c.cancelled_connects); return; },
        result = timeout(p.config.connect_timeout, connect_async_tls_with_config(request, Some(p.config.websocket_config()), true, Some(p.connector.clone()))) => result,
    };
    drop(permit);
    let mut ws = match result {
        Ok(Ok((ws, _))) => ws,
        Ok(Err(e)) => {
            c.error(format!("connect_{}", category(&e)));
            return;
        }
        Err(_) => {
            c.error("connect_timeout");
            return;
        }
    };
    Counters::record(&c.handshake, started.elapsed());
    c.opened();
    let _active = Active(c.clone());
    if let Err(error) = session(id, &mut ws, &p, &c, &stop).await
        && error != "cancelled"
    {
        c.error(error);
    }
    // A bounded close handshake also flushes any queued automatic protocol replies.
    let close = timeout(p.config.close_timeout, async {
        match ws.close(None).await {
            Ok(()) => (),
            Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => return Ok(()),
            Err(e) => return Err(e),
        }
        while let Some(message) = ws.next().await {
            match message {
                Ok(Message::Close(_)) | Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => {
                    return Ok(());
                }
                Err(e) => return Err(e),
                _ => (),
            }
        }
        Ok(())
    })
    .await;
    match close {
        Err(_) => c.error("close_timeout"),
        Ok(Err(e)) => c.error(format!("close_{}", category(&e))),
        Ok(Ok(())) => (),
    }
}

async fn session(
    id: u32,
    ws: &mut Socket,
    p: &Prepared,
    c: &Counters,
    stop: &CancellationToken,
) -> Result<(), String> {
    let period = match &p.config.behavior {
        Behavior::Silent => Duration::from_secs(3600),
        Behavior::PingPong { interval, .. } => *interval,
        Behavior::Flood(f) => Duration::from_secs_f64(1. / f.rate as f64),
    };
    let sending = !matches!(p.config.behavior, Behavior::Silent);
    let mut next_send = Instant::now();
    let mut pending: Option<(Vec<u8>, Instant)> = None;
    let mut sequence = 0u64;
    loop {
        let ping_deadline = match (&pending, &p.config.behavior) {
            (Some((_, at)), Behavior::PingPong { timeout, .. }) => *at + *timeout,
            _ => Instant::now() + Duration::from_secs(7 * 24 * 3600),
        };
        tokio::select! {
            _ = stop.cancelled() => {
                if pending.is_some() { inc(&c.pending_pings_at_stop); }
                return Ok(());
            }
            _ = sleep_until(ping_deadline), if pending.is_some() => return Err("pong_timeout".into()),
            message = ws.next() => {
                match message {
                    Some(Ok(Message::Text(text))) => { inc(&c.messages_received); add(&c.bytes_received, text.len()); }
                    Some(Ok(Message::Binary(data))) => { inc(&c.messages_received); add(&c.bytes_received, data.len()); }
                    Some(Ok(Message::Ping(_))) => {
                        inc(&c.pings_received);
                        // Tungstenite queues exactly one Pong with the original Ping payload.
                        write(ws.flush(), p.config.write_timeout, stop).await?;
                        inc(&c.pongs_sent);
                    }
                    Some(Ok(Message::Pong(data))) => {
                        inc(&c.pongs_received);
                        if let Some((expected, at)) = &pending
                            && expected.as_slice() == data.as_ref()
                        {
                            Counters::record(&c.rtt, at.elapsed());
                            pending = None;
                        }
                    }
                    Some(Ok(Message::Close(_))) => {
                        write(ws.flush(), p.config.write_timeout, stop).await?;
                        return Err("remote_closed".into());
                    }
                    Some(Err(e)) => return Err(format!("read_{}", category(&e))),
                    None => return Err("remote_eof".into()),
                    _ => (),
                }
            }
            _ = sleep_until(next_send), if sending && pending.is_none() => {
                if let Some(message) = &p.payload {
                    write(ws.send(message.clone()), p.config.write_timeout, stop).await?;
                    inc(&c.messages_sent);
                    add(&c.bytes_sent, message.len());
                } else {
                    sequence = sequence.wrapping_add(1);
                    let mut data = Vec::with_capacity(12);
                    data.extend_from_slice(&id.to_be_bytes());
                    data.extend_from_slice(&sequence.to_be_bytes());
                    let at = Instant::now();
                    write(ws.send(Message::Ping(data.clone().into())), p.config.write_timeout, stop).await?;
                    inc(&c.pings_sent);
                    pending = Some((data, at));
                }
                next_send += period;
                let now = Instant::now();
                if next_send < now {
                    if p.payload.is_some() {
                        let skipped = now.duration_since(next_send).as_nanos() / period.as_nanos() + 1;
                        c.missed_send_slots.fetch_add(skipped.min(u64::MAX as u128) as u64, Ordering::Relaxed);
                    }
                    // Never compensate for backpressure with a burst.
                    next_send = now + period;
                }
            }
        }
        // Yield even on a continuously readable socket so cancellation and other clients run.
        tokio::task::yield_now().await;
    }
}
async fn write(
    future: impl Future<Output = Result<(), WsError>>,
    limit: Duration,
    stop: &CancellationToken,
) -> Result<(), String> {
    tokio::select! {
        biased;
        _ = stop.cancelled() => Err("cancelled".into()),
        result = timeout(limit, future) => match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(format!("write_{}", category(&e))),
            Err(_) => Err("write_timeout".into()),
        }
    }
}
fn category(error: &WsError) -> String {
    match error {
        WsError::Io(e) => format!("io_{:?}", e.kind()).to_lowercase(),
        WsError::Tls(_) => "tls".into(),
        WsError::Http(r) => format!("http_{}", r.status().as_u16()),
        WsError::Protocol(_) => "protocol".into(),
        WsError::Capacity(_) => "capacity".into(),
        WsError::ConnectionClosed | WsError::AlreadyClosed => "closed".into(),
        WsError::Utf8 => "utf8".into(),
        _ => "websocket".into(),
    }
}
