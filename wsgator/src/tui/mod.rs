mod demo;
mod form;
mod state;
mod view;

use crate::report::LiveSnapshot;
use clap::{ArgMatches, parser::ValueSource};
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream, KeyEventKind, MouseButton, MouseEventKind,
    },
    execute,
};
use form::{Draft, Profile};
use futures::StreamExt;
use ratatui::DefaultTerminal;
use state::{Action, State, Tab};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::watch,
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};
use tokio_util::sync::CancellationToken;

struct Terminal {
    inner: DefaultTerminal,
}
impl Terminal {
    fn enter() -> Result<Self, String> {
        let terminal = Self {
            inner: ratatui::try_init().map_err(|e| format!("Cannot initialize terminal: {e}"))?,
        };
        execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture)
            .map_err(|e| e.to_string())?;
        Ok(terminal)
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste, DisableMouseCapture);
        ratatui::restore();
    }
}
struct Running {
    task: JoinHandle<Result<crate::report::Report, String>>,
    stop: CancellationToken,
    updates: watch::Receiver<LiveSnapshot>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop.cancel();
        self.task.abort();
    }
}

/// A persistent control deck. No-argument launches and `wsgator tui` share this entry.
pub async fn run(no_profile: bool, overrides: Option<&ArgMatches>) -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("Interactive mode needs a terminal. Run `wsgator --help` for headless commands or `wsgator tui` in a terminal.".into());
    }
    let profile = Profile::discover(no_profile);
    let loaded = profile.load();
    let mut draft = loaded.as_ref().cloned().unwrap_or_default();
    if let Some(matches) = overrides {
        seed(&mut draft, matches);
    }
    let mut app = State::new(draft);
    if let Err(error) = loaded {
        app.notice = error;
    }
    let mut terminal = Terminal::enter()?;
    let mut running: Option<Running> = None;
    let mut demo = None;
    let result = drive(
        &mut terminal.inner,
        &mut app,
        &mut running,
        &mut demo,
        &profile,
    )
    .await;
    // Always drain before restoring the screen, including terminal/input I/O failures.
    if let Some(mut current) = running.take() {
        current.stop.cancel();
        let _ = (&mut current.task).await;
    }
    if let Some(demo) = &mut demo {
        demo.shutdown().await;
    }
    if !app.demo_mode
        && app.draft.config().is_ok()
        && let Err(error) = profile.save(&app.draft)
    {
        drop(terminal);
        return result.and(Err(error));
    }
    drop(terminal);
    result
}
async fn drive(
    terminal: &mut DefaultTerminal,
    app: &mut State,
    running: &mut Option<Running>,
    demo: &mut Option<demo::Demo>,
    profile: &Profile,
) -> Result<(), String> {
    let mut events = EventStream::new();
    let mut tick = interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let signal = tokio::signal::ctrl_c();
    tokio::pin!(signal);
    let mut signal_received = false;
    loop {
        if let Some(current) = running.as_mut() {
            if current.updates.has_changed().unwrap_or(true) {
                app.observe(current.updates.borrow_and_update().clone());
            }
            if current.task.is_finished() {
                let mut finished = running.take().unwrap();
                app.observe(finished.updates.borrow_and_update().clone());
                app.finish(
                    (&mut finished.task)
                        .await
                        .unwrap_or_else(|e| Err(format!("Load task failed: {e}"))),
                );
                if app.quit_when_done {
                    return Ok(());
                }
            }
        }
        terminal
            .draw(|frame| view::draw(frame, app))
            .map_err(|e| format!("Cannot draw terminal: {e}"))?;
        let action = tokio::select! {
            event = events.next() => match event {
                Some(Ok(Event::Key(key))) if key.kind != KeyEventKind::Release => app.key(key),
                Some(Ok(Event::Paste(text))) => { app.paste(&text); Action::None }
                Some(Ok(Event::Mouse(event))) if !app.help && app.edit.is_none() => {
                    let point = (event.column, event.row).into();
                    match event.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            if let Some((tab,_)) = app.tab_hits.iter().find(|(_,rect)| rect.contains(point)) { app.tab = *tab; }
                            else if app.tab == Tab::Configure && !app.phase.busy() {
                                if let Some((mode,_)) = app.mode_hits.iter().find(|(_,rect)| rect.contains(point)) { app.draft.mode = *mode; app.focus = 0; }
                                else if let Some((field,_)) = app.field_hits.iter().find(|(_,rect)| rect.contains(point)) {
                                    app.focus = app.draft.fields(app.advanced).iter().position(|f| f == field).unwrap_or(0);
                                    app.edit_field();
                                }
                            }
                        }
                        MouseEventKind::ScrollDown if app.tab == Tab::Configure => app.move_focus(false),
                        MouseEventKind::ScrollUp if app.tab == Tab::Configure => app.move_focus(true),
                        MouseEventKind::ScrollDown if app.tab == Tab::Sessions => app.history_selection = (app.history_selection + 1).min(app.history.len().saturating_sub(1)),
                        MouseEventKind::ScrollUp if app.tab == Tab::Sessions => app.history_selection = app.history_selection.saturating_sub(1),
                        _ => (),
                    }
                    Action::None
                }
                Some(Ok(_)) => Action::None,
                Some(Err(e)) => return Err(format!("Terminal input failed: {e}")),
                None => return Err("Terminal input closed".into()),
            },
            _ = tick.tick() => Action::None,
            result = &mut signal, if !signal_received => {
                signal_received = true;
                result.map_err(|e| format!("Cannot listen for Ctrl+C: {e}"))?;
                if running.is_some() { app.quit_when_done = true; Action::Stop } else { Action::Quit }
            }
        };
        match action {
            Action::None => (),
            Action::Quit => return Ok(()),
            Action::Stop => {
                if let Some(current) = running {
                    current.stop.cancel();
                }
            }
            Action::Export => {
                if let Some(report) = app.selected_report() {
                    match export(&report) {
                        Ok(path) => app.notice = format!("Saved {}", path.display()),
                        Err(error) => app.notice = format!("Export failed: {error}"),
                    }
                } else {
                    app.notice =
                        "No report yet. Wait for a mission to finish before exporting.".into();
                }
            }
            Action::Demo => {
                if demo.is_none() {
                    match demo::Demo::start().await {
                        Ok(server) => *demo = Some(server),
                        Err(error) => {
                            app.notice = error;
                            continue;
                        }
                    }
                }
                let draft = Draft {
                    url: demo.as_ref().unwrap().url.clone(),
                    mode: form::Mode::Flood,
                    connections: "32".into(),
                    duration: "30s".into(),
                    rate: "20".into(),
                    ..Draft::default()
                };
                let config = draft.config()?;
                launch(app, running, profile, config, true);
                app.log(
                    "LOCAL DEMO  /  real loopback echo server, real traffic",
                    false,
                );
            }
            Action::Start(config) => {
                let is_demo = demo.as_ref().is_some_and(|server| {
                    server.url.trim_end_matches('/') == config.url.trim_end_matches('/')
                });
                launch(app, running, profile, *config, is_demo);
            }
        }
    }
}
fn launch(
    app: &mut State,
    running: &mut Option<Running>,
    profile: &Profile,
    mut config: crate::config::Config,
    is_demo: bool,
) {
    config.progress = false;
    config.json = None;
    app.begin(config.clone());
    app.demo_mode = is_demo;
    if !is_demo && let Err(error) = profile.save(&app.draft) {
        app.notice = error;
    }
    let stop = CancellationToken::new();
    let (sender, receiver) = watch::channel(LiveSnapshot::default());
    *running = Some(Running {
        task: tokio::spawn(crate::run_with_updates(config, stop.clone(), sender)),
        stop,
        updates: receiver,
    });
}

fn export(report: &crate::report::Report) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = PathBuf::from(format!("wsgator-report-{stamp}.json"));
    let mut bytes = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(&bytes).map_err(|e| e.to_string())?;
    Ok(path)
}
fn seed(draft: &mut Draft, matches: &ArgMatches) {
    if let Some(profile) = matches.get_one::<crate::LoadProfile>("load_profile") {
        draft.load_profile = profile.to_string();
    }
    for (id, field) in [
        ("url", &mut draft.url),
        ("subprotocol", &mut draft.subprotocol),
    ] {
        if matches.value_source(id) == Some(ValueSource::CommandLine)
            && let Some(value) = matches.get_one::<String>(id)
        {
            *field = value.clone();
        }
    }
    for (id, field) in [
        ("connections", &mut draft.connections),
        ("connect_concurrency", &mut draft.concurrency),
        ("max_message_bytes", &mut draft.max_message),
    ] {
        if matches.value_source(id) == Some(ValueSource::CommandLine)
            && let Some(value) = matches.get_one::<u32>(id)
        {
            *field = value.to_string();
        }
    }
    for (id, field) in [
        ("duration", &mut draft.duration),
        ("connect_timeout", &mut draft.connect_timeout),
        ("write_timeout", &mut draft.write_timeout),
        ("close_timeout", &mut draft.close_timeout),
    ] {
        if matches.value_source(id) == Some(ValueSource::CommandLine)
            && let Some(value) = matches.get_one::<Duration>(id)
        {
            *field = humantime::format_duration(*value).to_string();
        }
    }
    if let Some(value) = matches.get_one::<PathBuf>("ca_cert") {
        draft.ca_cert = value.to_string_lossy().into_owned();
    }
    if let Some(headers) = matches.get_many::<(
        tokio_tungstenite::tungstenite::http::HeaderName,
        tokio_tungstenite::tungstenite::http::HeaderValue,
    )>("headers")
    {
        draft.headers = headers
            .map(|(name, value)| format!("{name}: {}", value.to_str().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("\\n");
    }
}

#[cfg(test)]
mod tests;
