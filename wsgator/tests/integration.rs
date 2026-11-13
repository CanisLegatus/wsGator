use clap::Parser;
use futures::{SinkExt, StreamExt};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpListener,
    task::{JoinHandle, JoinSet},
    time::{sleep, timeout},
};
use tokio_tungstenite::{
    WebSocketStream, accept_async,
    tungstenite::{Error, Message},
};
use tokio_util::sync::CancellationToken;
use wsgator::{config::Config, report::Report};

#[derive(Clone, Copy)]
enum Mode {
    Echo,
    Heartbeat,
    IgnorePing,
    WrongPong,
    HangClose,
    Abrupt,
    RemoteClose,
    Oversized,
    NoRead,
}
struct Server {
    url: String,
    active: Arc<AtomicUsize>,
    received: Arc<AtomicUsize>,
    stop: CancellationToken,
    task: Option<JoinHandle<()>>,
}
struct Active(Arc<AtomicUsize>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
impl Server {
    async fn start(mode: Mode) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let active = Arc::new(AtomicUsize::new(0));
        let received = Arc::new(AtomicUsize::new(0));
        let stop = CancellationToken::new();
        let task = tokio::spawn({
            let active = active.clone();
            let received = received.clone();
            let stop = stop.clone();
            async move {
                let mut clients = JoinSet::new();
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        result = listener.accept() => {
                            let (stream, _) = result.unwrap();
                            let active = active.clone(); let received = received.clone();
                            clients.spawn(async move {
                                let ws = accept_async(stream).await.unwrap();
                                active.fetch_add(1, Ordering::SeqCst);
                                let _active = Active(active);
                                serve(ws, mode, received).await;
                            });
                        }
                        Some(result) = clients.join_next(), if !clients.is_empty() => { result.unwrap(); }
                    }
                }
                clients.abort_all();
                while clients.join_next().await.is_some() {}
            }
        });
        Self {
            url,
            active,
            received,
            stop,
            task: Some(task),
        }
    }
    async fn drained(&self) {
        timeout(Duration::from_secs(2), async {
            while self.active.load(Ordering::SeqCst) != 0 {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("server retained connections after client finished");
    }
}
async fn serve<S: AsyncRead + AsyncWrite + Unpin>(
    mut ws: WebSocketStream<S>,
    mode: Mode,
    received: Arc<AtomicUsize>,
) {
    if matches!(mode, Mode::Abrupt) {
        return;
    }
    if matches!(mode, Mode::NoRead) {
        sleep(Duration::from_secs(2)).await;
        return;
    }
    if matches!(mode, Mode::RemoteClose) {
        ws.close(None).await.unwrap();
    }
    if matches!(mode, Mode::Oversized) {
        let _ = ws.send(Message::Text("x".repeat(2048).into())).await;
    }
    if matches!(mode, Mode::Heartbeat) {
        ws.send(Message::Ping(b"server-heartbeat".to_vec().into()))
            .await
            .unwrap();
    }
    while let Some(message) = ws.next().await {
        match message {
            Ok(m @ (Message::Text(_) | Message::Binary(_))) => {
                received.fetch_add(1, Ordering::SeqCst);
                if ws.send(m).await.is_err() {
                    break;
                }
            }
            Ok(Message::Ping(_)) if matches!(mode, Mode::IgnorePing) => {
                // Do not poll the socket again: Tungstenite would flush its automatic Pong.
                sleep(Duration::from_secs(2)).await;
                break;
            }
            Ok(Message::Ping(_)) if matches!(mode, Mode::WrongPong) => {
                // Explicit Pong replaces Tungstenite's queued automatic Pong.
                if ws
                    .send(Message::Pong(b"wrong".to_vec().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Ok(Message::Ping(_)) => {
                if ws.flush().await.is_err() {
                    break;
                }
            }
            Ok(Message::Pong(data)) => {
                assert_eq!(data.as_ref(), b"server-heartbeat");
                received.fetch_add(1, Ordering::SeqCst);
            }
            Ok(Message::Close(_)) => {
                if matches!(mode, Mode::HangClose) {
                    sleep(Duration::from_secs(2)).await;
                } else {
                    let _ = ws.flush().await;
                }
                break;
            }
            Err(Error::ConnectionClosed) => break,
            Err(_) => break,
            _ => (),
        }
    }
}
fn config(url: &str, mode: &[&str]) -> Config {
    let mut args = vec![
        "wsgator",
        "--url",
        url,
        "-c",
        "3",
        "-d",
        "500ms",
        "--close-timeout",
        "100ms",
    ];
    args.extend_from_slice(mode);
    Config::try_parse_from(args).unwrap()
}
async fn run(config: Config) -> Report {
    timeout(
        Duration::from_secs(5),
        wsgator::run(config, CancellationToken::new()),
    )
    .await
    .expect("client run hung")
    .unwrap()
}

#[tokio::test]
async fn silent_holds_connections_reads_control_frames_and_drains() {
    let server = Server::start(Mode::Heartbeat).await;
    let report = run(config(&server.url, &["silent"])).await;
    assert!(!report.has_errors(), "{report:?}");
    assert_eq!(report.connected, 3);
    assert_eq!(report.peak_active, 3);
    assert_eq!(report.completed, 3);
    assert_eq!(report.active_at_end, 0);
    assert_eq!(report.messages_sent, 0);
    assert_eq!(report.pongs_sent, 3);
    assert_eq!(server.received.load(Ordering::SeqCst), 3);
    assert!(report.elapsed_seconds >= 0.49);
    server.drained().await;
}
#[tokio::test]
async fn ping_pong_correlates_round_trips() {
    let server = Server::start(Mode::Echo).await;
    let report = run(config(
        &server.url,
        &["ping-pong", "--interval", "40ms", "--timeout", "100ms"],
    ))
    .await;
    assert!(!report.has_errors(), "{report:?}");
    assert!(report.pings_sent >= 20, "{report:?}");
    assert_eq!(
        report.ping_rtt.as_ref().unwrap().samples,
        report.pongs_received
    );
    assert!(report.pings_sent - report.pongs_received <= 3);
    server.drained().await;
}
#[tokio::test]
async fn flood_is_paced_and_counts_echoed_payload() {
    let server = Server::start(Mode::Echo).await;
    let report = run(config(
        &server.url,
        &["flood", "--rate", "20", "--message", "hello"],
    ))
    .await;
    assert!(!report.has_errors(), "{report:?}");
    assert!((21..=33).contains(&report.messages_sent), "{report:?}");
    assert!(report.messages_received > 0);
    assert_eq!(report.bytes_sent, report.messages_sent * 5);
    assert_eq!(report.bytes_received, report.messages_received * 5);
    assert_eq!(
        report.messages_sent as usize,
        server.received.load(Ordering::SeqCst)
    );
    server.drained().await;
}
#[tokio::test]
async fn missing_and_wrong_pong_are_failures() {
    for mode in [Mode::IgnorePing, Mode::WrongPong] {
        let server = Server::start(mode).await;
        let report = run(config(
            &server.url,
            &["ping-pong", "--interval", "20ms", "--timeout", "60ms"],
        ))
        .await;
        assert_eq!(report.errors.get("pong_timeout"), Some(&3), "{report:?}");
        assert!(report.ping_rtt.is_none());
        assert_eq!(report.active_at_end, 0);
    }
}
#[tokio::test]
async fn close_handshake_is_bounded() {
    let server = Server::start(Mode::HangClose).await;
    let report = run(config(&server.url, &["silent"])).await;
    assert_eq!(report.errors.get("close_timeout"), Some(&3), "{report:?}");
    assert!(report.elapsed_seconds < 1.5);
    assert_eq!(report.active_at_end, 0);
}
#[tokio::test]
async fn server_disconnects_and_message_limits_are_reported() {
    for mode in [Mode::Abrupt, Mode::RemoteClose, Mode::Oversized] {
        let server = Server::start(mode).await;
        let mut cfg = config(&server.url, &["silent"]);
        cfg.max_message_bytes = 1024;
        let report = run(cfg).await;
        assert!(report.has_errors(), "{report:?}");
        assert_eq!(report.connected, 3);
        assert_eq!(report.active_at_end, 0);
        if matches!(mode, Mode::Oversized) {
            assert_eq!(report.errors.get("read_capacity"), Some(&3));
        }
        if matches!(mode, Mode::RemoteClose) {
            assert_eq!(report.errors.get("remote_closed"), Some(&3));
        }
        server.drained().await;
    }
}
#[tokio::test]
async fn refused_connections_cannot_produce_a_success_report() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    drop(listener);
    let report = run(config(&url, &["silent"])).await;
    assert_eq!(report.connected, 0);
    assert_eq!(report.attempted, 3);
    assert_eq!(report.errors.get("connect_io_connectionrefused"), Some(&3));
}
#[tokio::test]
async fn handshake_timeout_and_cancellation_include_queued_clients() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut streams = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            streams.push(stream);
        }
    });
    let mut cfg = config(&url, &["silent"]);
    cfg.connect_timeout = Duration::from_millis(50);
    let report = run(cfg).await;
    assert_eq!(report.errors.get("connect_timeout"), Some(&3));
    let mut cfg = config(&url, &["silent"]);
    cfg.duration = Duration::from_millis(50);
    cfg.connect_concurrency = 1;
    let report = run(cfg).await;
    assert_eq!(report.errors.get("load_incomplete"), Some(&1));
    assert_eq!(report.cancelled_connects, 1);
    assert_eq!(report.not_attempted, 2);
    let mut cfg = config(&url, &["silent"]);
    cfg.connect_concurrency = 1;
    cfg.duration = Duration::from_secs(30);
    let token = CancellationToken::new();
    let child = token.clone();
    let task = tokio::spawn(async move { wsgator::run(cfg, child).await.unwrap() });
    sleep(Duration::from_millis(80)).await;
    token.cancel();
    let report = timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert!(report.interrupted);
    assert_eq!(report.cancelled_connects, 1);
    assert_eq!(report.not_attempted, 2);
    assert!(!report.has_errors());
    server.abort();
}
#[tokio::test]
async fn cancellation_closes_active_clients() {
    let server = Server::start(Mode::Echo).await;
    let mut cfg = config(&server.url, &["flood", "--rate", "10"]);
    cfg.duration = Duration::from_secs(60);
    let stop = CancellationToken::new();
    let task = tokio::spawn(wsgator::run(cfg, stop.clone()));
    timeout(Duration::from_secs(2), async {
        while server.active.load(Ordering::SeqCst) < 3 || server.received.load(Ordering::SeqCst) < 3
        {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    stop.cancel();
    let report = timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(report.interrupted);
    assert!(!report.has_errors(), "{report:?}");
    assert_eq!(report.completed, report.connected);
    assert!(report.completed > 0);
    assert_eq!(
        report.completed + report.cancelled_connects,
        report.attempted
    );
    server.drained().await;
}

#[test]
fn cli_rejects_invalid_values_and_obsolete_strategies() {
    for args in [
        vec!["silent", "--url", "http://localhost"],
        vec!["silent", "--url", "ws://user:secret@localhost"],
        vec!["silent", "--duration", "0s"],
        vec!["silent", "--duration", "8days"],
        vec!["silent", "--connections", "0"],
        vec!["flood", "--rate", "0"],
        vec!["ping-pong", "--interval", "0s"],
        vec!["silent", "-H", "Upgrade: nope"],
        vec!["silent", "-H", "X-Test: bad\r\nInjected: yes"],
        vec!["silent", "--subprotocol", "two protocols"],
        vec!["flood", "--message", "hi", "--payload-file", "file"],
        vec!["sine"],
    ] {
        assert!(Config::try_parse_from(std::iter::once("wsgator").chain(args)).is_err());
    }
    assert!(
        Config::try_parse_from(["wsgator", "flood", "--payload-file", "file", "--binary"]).is_ok()
    );
}

struct TempFile(std::path::PathBuf);
impl TempFile {
    fn new(data: &[u8]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "wsgator-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, data).unwrap();
        Self(path)
    }
}
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[tokio::test]
async fn binary_payload_file_is_loaded_and_text_rejects_invalid_utf8() {
    let server = Server::start(Mode::Echo).await;
    let payload = TempFile::new(&[0, 255, 128, 1]);
    let path = payload.0.to_str().unwrap();
    let report = run(config(
        &server.url,
        &["flood", "--payload-file", path, "--binary", "--rate", "10"],
    ))
    .await;
    assert!(!report.has_errors(), "{report:?}");
    assert_eq!(report.behavior, "flood-binary");
    assert_eq!(report.payload_bytes, Some(4));
    assert_eq!(report.bytes_sent, report.messages_sent * 4);
    assert!(report.messages_received > 0);
    let error = wsgator::run(
        config(&server.url, &["flood", "--payload-file", path]),
        CancellationToken::new(),
    )
    .await
    .unwrap_err();
    assert!(error.contains("UTF-8"));
    server.drained().await;
}

#[tokio::test]
async fn headers_and_subprotocol_work_and_secrets_are_not_in_the_report() {
    use tokio_tungstenite::{
        accept_hdr_async,
        tungstenite::{
            handshake::server::{ErrorResponse, Request, Response},
            http::StatusCode,
        },
    };
    // Tungstenite's callback requires its unboxed HTTP ErrorResponse type.
    #[allow(clippy::result_large_err)]
    fn authenticate(req: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
        if req.headers().get("authorization").map(|v| v.as_bytes()) != Some(b"Bearer secret-header")
        {
            let mut error = ErrorResponse::new(Some("unauthorized".into()));
            *error.status_mut() = StatusCode::UNAUTHORIZED;
            return Err(error);
        }
        assert_eq!(req.headers()["sec-websocket-protocol"], "test.v1");
        response
            .headers_mut()
            .insert("sec-websocket-protocol", "test.v1".parse().unwrap());
        Ok(response)
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!(
        "ws://{}/socket?token=secret-query",
        listener.local_addr().unwrap()
    );
    let server = tokio::spawn(async move {
        let mut clients = JoinSet::new();
        for _ in 0..6 {
            let (stream, _) = listener.accept().await.unwrap();
            clients.spawn(async move {
                let ws = accept_hdr_async(stream, authenticate).await;
                if let Ok(ws) = ws {
                    serve(ws, Mode::Echo, Arc::new(AtomicUsize::new(0))).await;
                }
            });
        }
        while let Some(result) = clients.join_next().await {
            result.unwrap();
        }
    });
    let denied = run(config(&url, &["silent"])).await;
    assert_eq!(denied.errors.get("connect_http_401"), Some(&3));
    let accepted = run(config(
        &url,
        &[
            "silent",
            "-H",
            "Authorization: Bearer secret-header",
            "--subprotocol",
            "test.v1",
        ],
    ))
    .await;
    assert!(!accepted.has_errors(), "{accepted:?}");
    assert_eq!(accepted.connected, 3);
    let json = serde_json::to_string(&accepted).unwrap();
    assert!(!json.contains("secret"));
    assert!(!json.contains("token="));
    timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn tls_verifies_certificates_and_accepts_an_explicit_ca() {
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    let generated = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let ca = TempFile::new(generated.cert.pem().as_bytes());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(generated.key_pair.serialize_der()));
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![generated.cert.der().clone()], key)
    .unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(tls));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("wss://localhost:{}", listener.local_addr().unwrap().port());
    let server = tokio::spawn(async move {
        let mut clients = JoinSet::new();
        for _ in 0..6 {
            let (stream, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            clients.spawn(async move {
                if let Ok(stream) = acceptor.accept(stream).await {
                    let ws = accept_async(stream).await.unwrap();
                    serve(ws, Mode::Echo, Arc::new(AtomicUsize::new(0))).await;
                }
            });
        }
        while let Some(result) = clients.join_next().await {
            result.unwrap();
        }
    });
    let denied = run(config(&url, &["silent"])).await;
    assert_eq!(denied.connected, 0);
    assert!(denied.has_errors());
    let accepted = run(config(
        &url,
        &[
            "ping-pong",
            "--interval",
            "50ms",
            "--ca-cert",
            ca.0.to_str().unwrap(),
        ],
    ))
    .await;
    assert!(!accepted.has_errors(), "{accepted:?}");
    assert_eq!(accepted.connected, 3);
    assert!(accepted.ping_rtt.is_some());
    timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn cli_stdout_json_and_failure_exit_codes() {
    use tokio::process::Command;
    let server = Server::start(Mode::Echo).await;
    let output = timeout(
        Duration::from_secs(5),
        Command::new(env!("CARGO_BIN_EXE_wsgator"))
            .args([
                "flood",
                "--url",
                &server.url,
                "-c",
                "2",
                "-d",
                "200ms",
                "--json",
                "-",
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["connected"], 2);
    assert_eq!(json["active_at_end"], 0);
    assert!(json["errors"].as_object().unwrap().is_empty());
    let failing = Server::start(Mode::RemoteClose).await;
    let output = timeout(
        Duration::from_secs(5),
        Command::new(env!("CARGO_BIN_EXE_wsgator"))
            .args([
                "silent",
                "--url",
                &failing.url,
                "-c",
                "1",
                "-d",
                "150ms",
                "--json",
                "-",
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["errors"]["remote_closed"], 1);
    let output = Command::new(env!("CARGO_BIN_EXE_wsgator"))
        .args(["flood", "--rate", "0"])
        .output()
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    server.drained().await;
}

#[cfg(unix)]
#[tokio::test]
async fn cli_ctrl_c_drains_and_emits_partial_json() {
    use tokio::process::Command;
    let server = Server::start(Mode::Echo).await;
    let child = Command::new(env!("CARGO_BIN_EXE_wsgator"))
        .args([
            "flood",
            "--url",
            &server.url,
            "-c",
            "2",
            "-d",
            "60s",
            "--json",
            "-",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    timeout(Duration::from_secs(3), async {
        while server.active.load(Ordering::SeqCst) != 2
            || server.received.load(Ordering::SeqCst) < 2
        {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let result = Command::new("kill")
        .args(["-INT", &child.id().unwrap().to_string()])
        .status()
        .await
        .unwrap();
    assert!(result.success());
    let output = timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output.status.code(), Some(130));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["interrupted"], true);
    assert_eq!(json["active_at_end"], 0);
    assert_eq!(json["completed"], json["connected"]);
    assert!(json["completed"].as_u64().unwrap() > 0);
    server.drained().await;
}

#[tokio::test]
async fn a_non_reading_peer_cannot_block_writes_or_shutdown_forever() {
    let server = Server::start(Mode::NoRead).await;
    let mut cfg = config(&server.url, &["flood", "--rate", "100"]);
    cfg.connections = 1;
    cfg.write_timeout = Duration::from_millis(30);
    if let wsgator::config::Behavior::Flood(f) = &mut cfg.behavior {
        f.message = "x".repeat(8 * 1024 * 1024);
    }
    let report = run(cfg).await;
    assert_eq!(report.errors.get("write_timeout"), Some(&1), "{report:?}");
    assert_eq!(report.active_at_end, 0);
    assert!(report.elapsed_seconds < 1.5);
}
