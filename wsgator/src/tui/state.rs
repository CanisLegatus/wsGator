use super::form::{Draft, Field, Mode};
use crate::{
    config::Config,
    report::{LiveSnapshot, Report},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::{collections::VecDeque, sync::Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Configure,
    Telemetry,
    Sessions,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Ready,
    Starting,
    Running,
    Draining,
    Complete,
    Stopped,
    Failed,
}
impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ready => "STANDBY",
            Self::Starting => "CONNECTING",
            Self::Running => "LIVE",
            Self::Draining => "DRAINING",
            Self::Complete => "COMPLETE",
            Self::Stopped => "STOPPED",
            Self::Failed => "ATTENTION",
        }
    }
    pub fn busy(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::Draining)
    }
}
#[derive(Clone, Default)]
pub struct Sample {
    pub time: f64,
    pub active: f64,
    pub tx: f64,
    pub rx: f64,
    pub ping_tx: f64,
    pub pong_rx: f64,
    pub rtt: Option<f64>,
}
pub struct Log {
    pub time: f64,
    pub text: String,
    pub warning: bool,
}
pub struct Session {
    pub id: usize,
    pub config: Config,
    pub report: Arc<Report>,
    pub samples: Vec<Sample>,
}
pub struct Edit {
    pub original: String,
    pub cursor: usize,
}
pub enum Action {
    None,
    Start(Box<Config>),
    Stop,
    Quit,
    Export,
    Demo,
}
pub struct State {
    pub draft: Draft,
    pub tab: Tab,
    pub phase: Phase,
    pub advanced: bool,
    pub focus: usize,
    pub edit: Option<Edit>,
    pub live: LiveSnapshot,
    pub samples: VecDeque<Sample>,
    pub logs: VecDeque<Log>,
    pub history: VecDeque<Session>,
    pub history_selection: usize,
    pub viewing: Option<usize>,
    pub notice: String,
    pub help: bool,
    pub quit_when_done: bool,
    pub sequence: usize,
    pub demo_mode: bool,
    pub active_config: Option<Config>,
    pub report: Option<Arc<Report>>,
    pub field_hits: Vec<(Field, Rect)>,
    pub mode_hits: Vec<(Mode, Rect)>,
    pub tab_hits: Vec<(Tab, Rect)>,
}
impl State {
    pub fn new(draft: Draft) -> Self {
        Self {
            draft,
            tab: Tab::Configure,
            phase: Phase::Ready,
            advanced: false,
            focus: 0,
            edit: None,
            live: LiveSnapshot::default(),
            samples: VecDeque::new(),
            logs: VecDeque::new(),
            history: VecDeque::new(),
            history_selection: 0,
            viewing: None,
            notice: "Select a behavior, configure your target, then press F5 to launch.".into(),
            help: false,
            quit_when_done: false,
            sequence: 0,
            demo_mode: false,
            active_config: None,
            report: None,
            field_hits: Vec::new(),
            mode_hits: Vec::new(),
            tab_hits: Vec::new(),
        }
    }
    pub fn field(&self) -> Field {
        self.draft.fields(self.advanced)[self.focus.min(self.draft.fields(self.advanced).len() - 1)]
    }
    pub fn log(&mut self, text: impl Into<String>, warning: bool) {
        if self.logs.len() == 64 {
            self.logs.pop_front();
        }
        self.logs.push_back(Log {
            time: self.live.elapsed_seconds,
            text: text.into(),
            warning,
        });
    }
    pub fn begin(&mut self, config: Config) {
        self.sequence += 1;
        self.draft = Draft::from_config(&config);
        self.active_config = Some(config);
        self.phase = Phase::Starting;
        self.tab = Tab::Telemetry;
        self.viewing = None;
        self.report = None;
        self.live = LiveSnapshot::default();
        self.samples.clear();
        self.logs.clear();
        self.edit = None;
        self.log(
            "MISSION ARMED  /  establishing WebSocket connections",
            false,
        );
        self.notice = "Load engine online. Press x to stop gracefully; q stops and exits.".into();
    }
    pub fn observe(&mut self, next: LiveSnapshot) {
        if next.connected != self.live.connected {
            self.log(
                format!("CONNECTIONS  /  {} established", next.connected),
                false,
            );
        }
        for (kind, count) in &next.errors {
            if self.live.errors.get(kind) != Some(count) {
                self.log(format!("{kind}  /  {count} events"), true);
            }
        }
        if next.draining && self.phase != Phase::Draining {
            self.phase = Phase::Draining;
            self.log(
                "DRAIN  /  closing connections and collecting results",
                false,
            );
        } else if !next.finished && !next.draining && self.phase != Phase::Draining {
            self.phase = Phase::Running;
        }
        let delta = next.elapsed_seconds - self.live.elapsed_seconds;
        if delta > 0.01 && !next.draining && !next.finished {
            if self.samples.len() == 240 {
                self.samples.pop_front();
            }
            self.samples.push_back(Sample {
                time: next.elapsed_seconds,
                active: next.active as f64,
                tx: next.messages_sent.saturating_sub(self.live.messages_sent) as f64 / delta,
                rx: next
                    .messages_received
                    .saturating_sub(self.live.messages_received) as f64
                    / delta,
                ping_tx: next.pings_sent.saturating_sub(self.live.pings_sent) as f64 / delta,
                pong_rx: next.pongs_received.saturating_sub(self.live.pongs_received) as f64
                    / delta,
                rtt: next
                    .ping_rtt
                    .as_ref()
                    .or(next.handshake.as_ref())
                    .map(|latency| latency.p50_ms),
            });
        }
        self.live = next;
    }
    pub fn finish(&mut self, result: Result<Report, String>) {
        match result {
            Ok(report) => {
                self.phase = if report.interrupted {
                    Phase::Stopped
                } else if report.has_errors() {
                    Phase::Failed
                } else {
                    Phase::Complete
                };
                self.log(
                    format!(
                        "{}  /  {} connections completed, {} active remain",
                        self.phase.name(),
                        report.completed,
                        report.active_at_end
                    ),
                    report.has_errors(),
                );
                let report = Arc::new(report);
                if let Some(config) = self.active_config.clone() {
                    if self.history.len() == 20 {
                        self.history.pop_back();
                    }
                    self.history.push_front(Session {
                        id: self.sequence,
                        config,
                        report: report.clone(),
                        samples: self.samples.iter().cloned().collect(),
                    });
                }
                self.report = Some(report);
                self.history_selection = 0;
                self.notice = "Results retained. F5 reruns this mission · e exports JSON · 1 returns to setup.".into();
            }
            Err(error) => {
                self.phase = Phase::Failed;
                self.log(format!("SETUP FAILED  /  {error}"), true);
                self.notice = error;
            }
        }
    }
    pub fn shown_report(&self) -> Option<&Report> {
        self.viewing
            .and_then(|i| self.history.get(i))
            .map(|s| s.report.as_ref())
            .or(self.report.as_deref())
    }
    pub fn shown_config(&self) -> Option<&Config> {
        self.viewing
            .and_then(|i| self.history.get(i))
            .map(|s| &s.config)
            .or(self.active_config.as_ref())
    }
    pub fn shown_samples(&self) -> Vec<Sample> {
        self.viewing
            .and_then(|i| self.history.get(i))
            .map(|s| s.samples.clone())
            .unwrap_or_else(|| self.samples.iter().cloned().collect())
    }
    pub fn selected_report(&self) -> Option<Arc<Report>> {
        if self.tab == Tab::Sessions {
            self.history
                .get(self.history_selection)
                .map(|s| s.report.clone())
        } else {
            self.viewing
                .and_then(|i| self.history.get(i))
                .map(|s| s.report.clone())
                .or_else(|| self.report.clone())
        }
    }
    pub fn start_action(&mut self) -> Action {
        if self.phase.busy() {
            self.notice = "A mission is running. Stop it before starting another.".into();
            return Action::None;
        }
        let config = if self.tab == Tab::Sessions {
            self.history
                .get(self.history_selection)
                .map(|s| s.config.clone())
                .ok_or_else(|| "No sessions yet. Configure your first mission.".into())
        } else if self.tab == Tab::Telemetry {
            self.shown_config()
                .cloned()
                .ok_or_else(|| "Configure your first mission on the setup screen.".into())
        } else {
            self.draft.config()
        };
        match config {
            Ok(config) => Action::Start(Box::new(config)),
            Err(error) => {
                self.notice = error;
                Action::None
            }
        }
    }
    pub fn edit_field(&mut self) {
        if self.phase.busy() {
            self.notice = "Settings are locked while the engine is running.".into();
            return;
        }
        let field = self.field();
        if field == Field::Binary {
            self.draft.binary = !self.draft.binary;
        } else {
            let original = self.draft.value(field).to_string();
            self.edit = Some(Edit {
                cursor: original.len(),
                original,
            });
        }
    }
    pub fn move_focus(&mut self, backward: bool) {
        self.edit = None;
        let len = self.draft.fields(self.advanced).len();
        self.focus = if backward {
            (self.focus + len - 1) % len
        } else {
            (self.focus + 1) % len
        };
    }
    pub fn paste(&mut self, text: &str) {
        if self.edit.is_none() {
            return;
        }
        let text = if self.field() == Field::Headers {
            text.replace('\r', "").replace('\n', "\\n")
        } else {
            text.replace(['\r', '\n'], " ")
        };
        self.insert(&text);
    }
    fn insert(&mut self, text: &str) {
        let field = self.field();
        if let Some(edit) = &mut self.edit
            && let Some(value) = self.draft.value_mut(field)
        {
            if value.len() + text.len() <= 16_384 {
                value.insert_str(edit.cursor, text);
                edit.cursor += text.len();
            } else {
                self.notice =
                    "Input limit is 16 KiB. Use a payload file for larger messages.".into();
            }
        }
    }
    pub fn key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.phase.busy() {
                self.quit_when_done = true;
                self.phase = Phase::Draining;
                return Action::Stop;
            }
            return Action::Quit;
        }
        if self.help {
            if matches!(key.code, KeyCode::Esc | KeyCode::Char('?')) {
                self.help = false;
            }
            return Action::None;
        }
        if self.edit.is_some() {
            self.edit_key(key);
            return Action::None;
        }
        match key.code {
            KeyCode::Char('1') => self.tab = Tab::Configure,
            KeyCode::Char('2') => self.tab = Tab::Telemetry,
            KeyCode::Char('3') => self.tab = Tab::Sessions,
            KeyCode::Char('?') | KeyCode::F(1) => self.help = true,
            KeyCode::Char('q') => {
                if self.phase.busy() {
                    self.quit_when_done = true;
                    self.phase = Phase::Draining;
                    return Action::Stop;
                }
                return Action::Quit;
            }
            KeyCode::F(5) | KeyCode::Char('r') => return self.start_action(),
            KeyCode::Char('d') if !self.phase.busy() => return Action::Demo,
            KeyCode::Char('x') | KeyCode::F(6) if self.phase.busy() => {
                self.phase = Phase::Draining;
                self.notice = "Stopping gracefully. Waiting for client close handshakes…".into();
                return Action::Stop;
            }
            KeyCode::Char('e') if self.tab != Tab::Configure => return Action::Export,
            _ => match self.tab {
                Tab::Configure => match key.code {
                    KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => self.move_focus(false),
                    KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => self.move_focus(true),
                    KeyCode::Enter | KeyCode::Char(' ') => self.edit_field(),
                    KeyCode::Char('a') => {
                        self.advanced = !self.advanced;
                        self.focus = 0;
                    }
                    KeyCode::Left | KeyCode::Right | KeyCode::Char('m') if !self.phase.busy() => {
                        self.draft.mode = self.draft.mode.next(key.code == KeyCode::Left);
                        self.focus = 0;
                    }
                    _ => (),
                },
                Tab::Sessions => match key.code {
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.history_selection =
                            (self.history_selection + 1).min(self.history.len().saturating_sub(1))
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.history_selection = self.history_selection.saturating_sub(1)
                    }
                    KeyCode::Enter if !self.history.is_empty() => {
                        self.viewing = Some(self.history_selection);
                        self.tab = Tab::Telemetry;
                    }
                    _ => (),
                },
                Tab::Telemetry => (),
            },
        }
        Action::None
    }
    fn edit_key(&mut self, key: KeyEvent) {
        let field = self.field();
        if key.code == KeyCode::Esc {
            if let Some(edit) = self.edit.take()
                && let Some(value) = self.draft.value_mut(field)
            {
                *value = edit.original;
            }
            return;
        }
        if key.code == KeyCode::Enter {
            self.edit = None;
            return;
        }
        if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
            self.move_focus(key.code == KeyCode::BackTab);
            return;
        }
        if key.code == KeyCode::Char('u') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if let Some(value) = self.draft.value_mut(field) {
                value.clear();
            }
            if let Some(edit) = &mut self.edit {
                edit.cursor = 0;
            }
            return;
        }
        if let KeyCode::Char(c) = key.code
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            self.insert(&c.to_string());
            return;
        }
        if let Some(edit) = &mut self.edit
            && let Some(value) = self.draft.value_mut(field)
        {
            match key.code {
                KeyCode::Home => edit.cursor = 0,
                KeyCode::End => edit.cursor = value.len(),
                KeyCode::Left => {
                    edit.cursor = value[..edit.cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0)
                }
                KeyCode::Right => {
                    edit.cursor += value[edit.cursor..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(0)
                }
                KeyCode::Backspace if edit.cursor > 0 => {
                    let previous = value[..edit.cursor].char_indices().next_back().unwrap().0;
                    value.drain(previous..edit.cursor);
                    edit.cursor = previous;
                }
                KeyCode::Delete if edit.cursor < value.len() => {
                    let next =
                        edit.cursor + value[edit.cursor..].chars().next().unwrap().len_utf8();
                    value.drain(edit.cursor..next);
                }
                _ => (),
            }
        }
    }
}
