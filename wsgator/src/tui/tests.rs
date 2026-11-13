use super::form::{Field, Mode};
use super::state::{Phase, Tab};
use super::*;
use crate::{
    config::{Behavior, Config},
    report::{LiveSnapshot, Report},
};
use clap::Parser;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicUsize, Ordering},
};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn report() -> Report {
    Report {
        schema_version: 1,
        version: "test",
        target: "ws://127.0.0.1:9001/".into(),
        behavior: "ping-pong".into(),
        requested_connections: 10,
        connect_concurrency: 256,
        connect_timeout_seconds: 10.,
        write_timeout_seconds: 5.,
        close_timeout_seconds: 2.,
        max_message_bytes: 1_048_576,
        subprotocol: None,
        configured_duration_seconds: 30.,
        load_duration_seconds: 30.,
        elapsed_seconds: 30.01,
        interrupted: false,
        payload_bytes: None,
        messages_per_second_per_connection: None,
        ping_interval_seconds: Some(1.),
        pong_timeout_seconds: Some(5.),
        attempted: 10,
        connected: 10,
        peak_active: 10,
        active_at_end: 0,
        completed: 10,
        cancelled_connects: 0,
        not_attempted: 0,
        messages_sent: 0,
        messages_received: 0,
        bytes_sent: 0,
        bytes_received: 0,
        pings_sent: 300,
        pings_received: 0,
        pongs_received: 300,
        pongs_sent: 0,
        pending_pings_at_stop: 0,
        missed_send_slots: 0,
        send_messages_per_second: 0.,
        receive_messages_per_second: 0.,
        handshake: None,
        ping_rtt: None,
        errors: BTreeMap::new(),
    }
}
fn render(app: &mut State, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| view::draw(f, app)).unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn fields_share_cli_validation_and_respect_mode_options() {
    let mut draft = Draft::default();
    assert!(matches!(
        draft.config().unwrap().behavior,
        Behavior::PingPong { .. }
    ));
    draft.connections = "0".into();
    assert!(draft.config().is_err());
    draft.connections = "5".into();
    draft.mode = Mode::Flood;
    draft.binary = true;
    draft.payload_file = "test.bin".into();
    draft.headers = "Authorization: Bearer token\\nX-Test: yes".into();
    let c = draft.config().unwrap();
    assert_eq!(c.headers.len(), 2);
    assert!(matches!(c.behavior,Behavior::Flood(ref f) if f.binary && f.payload_file.is_some()));
    assert!(draft.fields(false).contains(&Field::PayloadFile));
    assert!(!draft.fields(false).contains(&Field::CaCert));
    assert!(draft.fields(true).contains(&Field::CaCert));
}
#[test]
fn unicode_editing_paste_and_escape_preserve_character_boundaries() {
    let mut app = State::new(Draft::default());
    app.draft.url.clear();
    app.key(key(KeyCode::Enter));
    app.paste("hé🐊");
    app.key(key(KeyCode::Left));
    app.key(key(KeyCode::Backspace));
    assert_eq!(app.draft.url, "h🐊");
    app.key(key(KeyCode::Delete));
    assert_eq!(app.draft.url, "h");
    app.key(key(KeyCode::Esc));
    assert_eq!(app.draft.url, "");
    assert!(app.edit.is_none());
    app.key(key(KeyCode::Enter));
    app.paste("ws://localhost:9001");
    app.key(key(KeyCode::Enter));
    assert_eq!(app.draft.url, "ws://localhost:9001");
    app.key(key(KeyCode::Enter));
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    assert!(app.draft.url.is_empty());
    // Rendering a cleared or horizontally scrolled field must remain safe.
    render(&mut app, 80, 24);
    app.paste(&"é".repeat(200));
    render(&mut app, 80, 24);
}
#[test]
fn mode_changes_and_validation_do_not_launch_bad_missions() {
    let mut app = State::new(Draft::default());
    app.key(key(KeyCode::Right));
    assert_eq!(app.draft.mode, Mode::Flood);
    app.key(key(KeyCode::Left));
    assert_eq!(app.draft.mode, Mode::PingPong);
    app.draft.url = "http://bad".into();
    assert!(matches!(app.key(key(KeyCode::F(5))), Action::None));
    assert!(app.notice.contains("ws://"));
    app.draft.url = "ws://localhost:9001".into();
    assert!(matches!(app.key(key(KeyCode::F(5))), Action::Start(_)));
}
#[test]
fn live_rates_use_observation_time_and_keep_bounded_history() {
    let mut app = State::new(Draft::default());
    app.begin(app.draft.config().unwrap());
    for i in 1..=300 {
        app.observe(LiveSnapshot {
            elapsed_seconds: i as f64 / 4.,
            connected: 10,
            active: 10,
            messages_sent: i * 5,
            messages_received: i * 4,
            ..LiveSnapshot::default()
        });
    }
    assert_eq!(app.samples.len(), 240);
    let last = app.samples.back().unwrap();
    assert_eq!(last.tx, 20.);
    assert_eq!(last.rx, 16.);
    assert!(app.logs.len() <= 64);
    app.observe(LiveSnapshot {
        elapsed_seconds: 75.,
        draining: true,
        ..app.live.clone()
    });
    assert_eq!(app.phase, Phase::Draining);
    app.finish(Ok(report()));
    assert_eq!(app.phase, Phase::Complete);
    assert_eq!(app.history.len(), 1);
    assert!(matches!(app.key(key(KeyCode::F(5))), Action::Start(_)));
    assert!(matches!(app.key(key(KeyCode::Char('q'))), Action::Quit));
}
#[test]
fn stopping_quitting_and_failures_keep_the_control_deck_alive() {
    let mut app = State::new(Draft::default());
    app.begin(app.draft.config().unwrap());
    assert!(matches!(app.key(key(KeyCode::Char('q'))), Action::Stop));
    assert!(app.quit_when_done);
    assert_eq!(app.phase, Phase::Draining);
    app.finish(Err("cannot read payload".into()));
    assert_eq!(app.phase, Phase::Failed);
    assert!(app.notice.contains("payload"));
    assert!(app.history.is_empty());
}
#[test]
fn profiles_restore_settings_without_headers_payload_or_query_tokens() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let directory = std::env::temp_dir().join(format!(
        "wsgator-profile-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let profile = Profile {
        path: Some(directory.join("profile.json")),
    };
    let draft = Draft {
        url: "wss://example.com/socket?token=secret-query".into(),
        headers: "Authorization: secret-header".into(),
        message: "secret-message".into(),
        connections: "42".into(),
        ..Draft::default()
    };
    profile.save(&draft).unwrap();
    let bytes = std::fs::read_to_string(profile.path.as_ref().unwrap()).unwrap();
    assert!(!bytes.contains("secret"));
    let restored = profile.load().unwrap();
    assert_eq!(restored.connections, "42");
    assert_eq!(restored.url, "wss://example.com/socket");
    assert!(restored.headers.is_empty());
    assert_eq!(restored.message, "wsGator");
    assert_eq!(draft.headers, "Authorization: secret-header");
    assert!(Profile::discover(true).path.is_none());
    std::fs::write(profile.path.as_ref().unwrap(), "broken").unwrap();
    assert!(profile.load().is_err());
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn layouts_render_all_modes_pages_and_resizes_without_losing_controls() {
    for (width, height) in [(60, 15), (72, 24), (80, 24), (128, 42), (180, 55)] {
        for mode in [Mode::Silent, Mode::PingPong, Mode::Flood] {
            let mut app = State::new(Draft {
                mode,
                ..Draft::default()
            });
            let text = render(&mut app, width, height);
            assert!(text.contains("wsGator"));
            if width >= 72 {
                assert!(text.contains("CONFIGURE"));
                assert!(text.contains("TARGET ENDPOINT"));
                app.advanced = true;
                app.focus = app.draft.fields(true).len() - 1;
                assert!(render(&mut app, width, height).contains("HTTP HEADERS"));
            }
            app.tab = Tab::Telemetry;
            render(&mut app, width, height);
            app.tab = Tab::Sessions;
            render(&mut app, width, height);
            app.help = true;
            render(&mut app, width, height);
        }
    }
}
#[test]
fn completed_sessions_can_be_inspected_and_rerun_with_original_settings() {
    let mut app = State::new(Draft::default());
    for _ in 0..25 {
        let config = Config::try_parse_from(["wsgator", "ping-pong", "-c", "10"]).unwrap();
        app.begin(config);
        app.finish(Ok(report()));
    }
    assert_eq!(app.history.len(), 20);
    app.tab = Tab::Sessions;
    app.history_selection = 1;
    app.key(key(KeyCode::Enter));
    assert_eq!(app.tab, Tab::Telemetry);
    assert_eq!(app.shown_report().unwrap().requested_connections, 10);
    app.draft.connections = "999".into();
    match app.start_action() {
        Action::Start(config) => assert_eq!(config.connections, 10),
        _ => panic!("expected rerun"),
    }
    assert!(render(&mut app, 128, 42).contains("ARCHIVED"));
}

#[test]
fn heartbeat_dashboard_uses_control_frames_instead_of_application_traffic() {
    let mut app = State::new(Draft::default());
    app.begin(app.draft.config().unwrap());
    app.observe(LiveSnapshot {
        elapsed_seconds: 0.25,
        connected: 10,
        active: 10,
        pings_sent: 50,
        pongs_received: 48,
        ..LiveSnapshot::default()
    });
    assert_eq!(app.samples.back().unwrap().tx, 0.);
    let text = render(&mut app, 128, 42);
    assert!(text.contains("HEARTBEAT FLOW"));
    assert!(text.contains("200 / 192"));
    assert!(text.contains("PING RTT"));
}

#[test]
fn explicit_tui_arguments_override_only_the_supplied_profile_settings() {
    use clap::CommandFactory;
    let matches = Config::command()
        .subcommand(clap::Command::new("tui"))
        .try_get_matches_from([
            "wsgator",
            "tui",
            "--url",
            "wss://example.com/socket",
            "-c",
            "7",
            "-d",
            "2s",
            "--header",
            "X-Test: yes",
        ])
        .unwrap();
    let mut draft = Draft {
        rate: "42".into(),
        ..Draft::default()
    };
    seed(&mut draft, &matches);
    assert_eq!(draft.url, "wss://example.com/socket");
    assert_eq!(draft.connections, "7");
    assert_eq!(draft.duration, "2s");
    assert_eq!(draft.headers, "x-test: yes");
    assert_eq!(draft.rate, "42");
    assert!(draft.config().is_ok());
}
