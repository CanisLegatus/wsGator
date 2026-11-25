use super::{
    form::{Field, Mode},
    state::{Phase, Sample, State, Tab},
};
use crate::{PhaseKind, load::LoadPlan};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        Axis, Block, BorderType, Borders, Cell, Chart, Clear, Dataset, Gauge, GraphType, Paragraph,
        Row, Sparkline, Table, TableState, Wrap,
    },
};
use unicode_width::UnicodeWidthStr;

pub const BG: Color = Color::Rgb(8, 13, 22);
pub const PANEL: Color = Color::Rgb(13, 22, 34);
pub const EDGE: Color = Color::Rgb(47, 67, 86);
pub const TEXT: Color = Color::Rgb(213, 229, 240);
pub const MUTED: Color = Color::Rgb(111, 136, 157);
pub const GREEN: Color = Color::Rgb(160, 235, 105);
pub const CYAN: Color = Color::Rgb(73, 218, 230);
pub const AMBER: Color = Color::Rgb(255, 184, 108);
pub const RED: Color = Color::Rgb(255, 113, 143);
const PURPLE: Color = Color::Rgb(176, 157, 255);
fn color(mode: Mode) -> Color {
    match mode {
        Mode::Silent => GREEN,
        Mode::PingPong => CYAN,
        Mode::Flood => AMBER,
    }
}
fn bold(color: Color) -> Style {
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}
fn panel(title: impl Into<Line<'static>>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(EDGE))
        .style(Style::default().bg(PANEL))
        .title(title)
        .title_style(Style::default().fg(MUTED))
}
fn title(id: &str, label: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {id} "), bold(CYAN)),
        Span::styled(format!("{label} "), Style::default().fg(TEXT)),
    ])
}
fn count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}m", n as f64 / 1_000_000.)
    } else if n >= 10_000 {
        format!("{:.1}k", n as f64 / 1000.)
    } else {
        n.to_string()
    }
}
fn rate(n: f64) -> String {
    if n >= 1_000_000. {
        format!("{:.1}m", n / 1_000_000.)
    } else if n >= 1000. {
        format!("{:.1}k", n / 1000.)
    } else {
        format!("{n:.0}")
    }
}
fn duration(n: f64) -> String {
    let n = n.max(0.) as u64;
    format!("{:02}:{:02}", n / 60, n % 60)
}
fn latency(n: Option<f64>) -> String {
    n.map(|v| format!("{v:.2} ms"))
        .unwrap_or_else(|| "—".into())
}
fn phase_color(phase: Phase) -> Color {
    match phase {
        Phase::Failed => RED,
        Phase::Draining | Phase::Stopped => AMBER,
        Phase::Complete | Phase::Ready => GREEN,
        _ => CYAN,
    }
}

pub fn draw(frame: &mut Frame, app: &mut State) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(BG).fg(TEXT)),
        area,
    );
    app.field_hits.clear();
    app.mode_hits.clear();
    app.tab_hits.clear();
    if area.width < 72 || area.height < 24 {
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(Span::styled("wsGator / CONTROL DECK", bold(CYAN))),
                Line::from(""),
                Line::from("Resize the terminal to at least 72 × 24."),
                Line::from(format!("Current size: {} × {}", area.width, area.height)),
                Line::from(""),
                Line::from("q  quit safely     x  stop the current mission"),
            ]))
            .alignment(Alignment::Center)
            .block(panel(" TERMINAL SIZE ")),
            area,
        );
        return;
    }
    let outer = area.inner(ratatui::layout::Margin::new(1, 0));
    let regions = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Min(10),
        Constraint::Length(3),
    ])
    .split(outer);
    header(frame, app, regions[0]);
    navigation(frame, app, regions[1]);
    match app.tab {
        Tab::Configure => configure(frame, app, regions[2]),
        Tab::Telemetry => telemetry(frame, app, regions[2]),
        Tab::Sessions => sessions(frame, app, regions[2]),
    }
    footer(frame, app, regions[3]);
    if app.help {
        help(frame, area);
    }
}
fn header(frame: &mut Frame, app: &State, area: Rect) {
    let parts = Layout::horizontal([Constraint::Min(40), Constraint::Length(25)]).split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" ▰ ", bold(GREEN)),
                Span::styled("wsGator", bold(TEXT)),
                Span::styled("  /  CONTROL DECK", Style::default().fg(CYAN)),
            ]),
            Line::from(Span::styled(
                format!(
                    "     WEBSOCKET LOAD LAB    ·    v{}",
                    env!("CARGO_PKG_VERSION")
                ),
                Style::default().fg(MUTED),
            )),
        ]),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!("● {}  ", app.phase.name()),
                bold(phase_color(app.phase)),
            )),
            Line::from(Span::styled(
                if app.demo_mode {
                    "LOCAL DEMO / LOOPBACK  "
                } else {
                    "SINGLE NODE  /  FLAT LOAD  "
                },
                Style::default().fg(MUTED),
            )),
        ])
        .alignment(Alignment::Right),
        parts[1],
    );
}
fn navigation(frame: &mut Frame, app: &mut State, area: Rect) {
    let chunks = Layout::horizontal([
        Constraint::Length(22),
        Constraint::Length(22),
        Constraint::Length(22),
        Constraint::Min(0),
    ])
    .split(area);
    for (i, (tab, name)) in [
        (Tab::Configure, "CONFIGURE"),
        (Tab::Telemetry, "TELEMETRY"),
        (Tab::Sessions, "SESSIONS"),
    ]
    .into_iter()
    .enumerate()
    {
        let style = if app.tab == tab {
            bold(BG).bg(CYAN)
        } else {
            Style::default().fg(MUTED)
        };
        let label = if tab == Tab::Sessions {
            format!(" {}  {} ({}) ", i + 1, name, app.history.len())
        } else {
            format!(" {}  {name} ", i + 1)
        };
        frame.render_widget(
            Paragraph::new(label).style(style),
            Rect {
                height: 1,
                ..chunks[i]
            },
        );
        app.tab_hits.push((tab, chunks[i]));
    }
}
fn configure(frame: &mut Frame, app: &mut State, area: Rect) {
    let columns = Layout::horizontal([
        Constraint::Length(if area.width >= 100 { 30 } else { 24 }),
        Constraint::Min(40),
    ])
    .spacing(1)
    .split(area);
    let sidebar = columns[0];
    let items = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Length(5),
        Constraint::Min(0),
    ])
    .split(sidebar);
    frame.render_widget(
        Paragraph::new("SELECT LOAD BEHAVIOR").style(bold(MUTED)),
        items[0],
    );
    for (i, (mode, subtitle, desc)) in [
        (
            Mode::Silent,
            "CONNECTION HOLD",
            "Keep sockets open.\nRead traffic & answer Ping.",
        ),
        (
            Mode::PingPong,
            "HEARTBEAT PROBE",
            "Send Ping, verify Pong.\nMeasure protocol RTT.",
        ),
        (
            Mode::Flood,
            "PACED MESSAGE STREAM",
            "Push text or binary data.\nControl each client's rate.",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let selected = app.draft.mode == mode;
        let accent = color(mode);
        let block = panel(Line::from(Span::styled(
            format!(" {}  {} ", if selected { "◆" } else { "◇" }, mode.name()),
            if selected {
                bold(accent)
            } else {
                Style::default().fg(MUTED)
            },
        )))
        .border_style(Style::default().fg(if selected { accent } else { EDGE }));
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    subtitle,
                    Style::default().fg(if selected { TEXT } else { MUTED }),
                )),
                Line::from(Span::styled(
                    desc.split('\n').next().unwrap_or_default(),
                    Style::default().fg(MUTED),
                )),
                Line::from(Span::styled(
                    desc.split('\n').nth(1).unwrap_or_default(),
                    Style::default().fg(MUTED),
                )),
            ])
            .block(block),
            items[i + 1],
        );
        app.mode_hits.push((mode, items[i + 1]));
    }
    if items[4].height >= 4 {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled("BUILT TO BITE.", bold(GREEN))),
                Line::from(""),
                Line::from("One engine. Real sockets."),
                Line::from(""),
                Line::from(Span::styled("[ d ] TRY LOCAL DEMO", bold(CYAN))),
                Line::from("No server setup needed."),
            ])
            .style(Style::default().fg(MUTED))
            .block(panel(" ws:// ")),
            items[4],
        );
    }
    let form = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(6),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .split(columns[1]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("MISSION PARAMETERS", bold(TEXT)),
            Span::styled(
                if app.advanced {
                    "   ADVANCED OPEN  [a]"
                } else {
                    "   ADVANCED  [a]"
                },
                Style::default().fg(CYAN),
            ),
        ])),
        form[0],
    );
    let fields = app.draft.fields(app.advanced);
    let mut rows: Vec<Vec<Field>> = vec![vec![fields[0]]];
    rows.extend(fields[1..].chunks(2).map(|chunk| chunk.to_vec()));
    let visible_rows = (form[1].height / 3).max(1) as usize;
    let focus_row = app.focus.div_ceil(2);
    let offset = focus_row.saturating_add(1).saturating_sub(visible_rows);
    for (row, row_fields) in rows.iter().skip(offset).take(visible_rows).enumerate() {
        let rect = Rect::new(
            form[1].x,
            form[1].y + row as u16 * 3,
            form[1].width,
            3.min(form[1].height.saturating_sub(row as u16 * 3)),
        );
        let cells = Layout::horizontal(vec![
            Constraint::Ratio(1, row_fields.len() as u32);
            row_fields.len()
        ])
        .spacing(1)
        .split(rect);
        for (field, rect) in row_fields.iter().zip(cells.iter()) {
            field_widget(frame, app, *field, *rect);
        }
    }
    let consumed = rows.len().min(visible_rows) as u16 * 3;
    if offset == 0 && form[1].height.saturating_sub(consumed) >= 7 {
        let preview = Rect::new(form[1].x, form[1].y + consumed + 1, form[1].width, 6);
        let block = panel(title("//", "MISSION PREVIEW"));
        let inner = block.inner(preview);
        frame.render_widget(block, preview);
        let cells = Layout::horizontal([Constraint::Ratio(1, 3); 3])
            .spacing(1)
            .split(inner);
        let plan = app.draft.config().ok().as_ref().map(LoadPlan::new);
        let clients = plan.as_ref().map(|plan| u64::from(plan.connections));
        let nominal = match app.draft.mode {
            Mode::Silent => "LISTEN ONLY".into(),
            Mode::Flood => clients
                .zip(app.draft.rate.parse::<u64>().ok())
                .map(|(c, r)| format!("{} msg/s", count(c.saturating_mul(r))))
                .unwrap_or_else(|| "—".into()),
            Mode::PingPong => clients
                .zip(humantime::parse_duration(&app.draft.interval).ok())
                .filter(|(_, d)| !d.is_zero())
                .map(|(c, d)| format!("{} Ping/s", rate(c as f64 / d.as_secs_f64())))
                .unwrap_or_else(|| "—".into()),
        };
        for ((label, value), cell) in [
            ("CLIENTS", clients.map(count).unwrap_or_else(|| "—".into())),
            (
                "WINDOW",
                plan.map(|plan| humantime::format_duration(plan.duration).to_string())
                    .unwrap_or_else(|| "—".into()),
            ),
            ("NOMINAL TX", nominal),
        ]
        .into_iter()
        .zip(cells.iter())
        {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(label, Style::default().fg(MUTED))),
                    Line::from(""),
                    Line::from(Span::styled(value, bold(color(app.draft.mode)))),
                ]),
                *cell,
            );
        }
    }
    let hint = if app.phase.busy() {
        "Mission in progress. Stop the engine before changing settings."
    } else {
        app.field().hint()
    };
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(MUTED))
            .wrap(Wrap { trim: true }),
        form[2],
    );
    let action = if app.phase.busy() {
        " [ x ] STOP MISSION "
    } else {
        " [ F5 ] LAUNCH MISSION "
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                action,
                bold(BG).bg(if app.phase.busy() { AMBER } else { GREEN }),
            ),
            Span::styled(
                "   ← → behavior   ·   Tab focus   ·   Enter edit",
                Style::default().fg(MUTED),
            ),
        ])),
        form[3],
    );
}
fn field_widget(frame: &mut Frame, app: &mut State, field: Field, rect: Rect) {
    let selected = app.field() == field;
    let editing = selected && app.edit.is_some();
    let value = app.draft.value(field);
    let value = if field == Field::Headers && !editing && !value.is_empty() {
        "••••••••  (session only)"
    } else if value.is_empty() && !editing {
        "not set"
    } else {
        value
    };
    let border = if editing {
        GREEN
    } else if selected {
        CYAN
    } else {
        EDGE
    };
    let label = format!(
        " {}{} ",
        if editing { "EDIT · " } else { "" },
        field.label()
    );
    let block = panel(Line::from(Span::styled(
        label,
        Style::default().fg(if selected { border } else { MUTED }),
    )))
    .border_style(Style::default().fg(border));
    let cursor_width = app
        .edit
        .as_ref()
        .filter(|_| editing)
        .map(|edit| UnicodeWidthStr::width(&value[..edit.cursor]))
        .unwrap_or(0);
    let scroll = cursor_width.saturating_sub(rect.width.saturating_sub(3) as usize);
    frame.render_widget(
        Paragraph::new(value)
            .style(Style::default().fg(if selected { TEXT } else { MUTED }))
            .scroll((0, scroll.min(u16::MAX as usize) as u16))
            .block(block),
        rect,
    );
    if editing && rect.width > 2 && rect.height > 2 {
        frame.set_cursor_position((rect.x + 1 + (cursor_width - scroll) as u16, rect.y + 1));
    }
    app.field_hits.push((field, rect));
}

fn telemetry(frame: &mut Frame, app: &State, area: Rect) {
    let sections = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(2),
        Constraint::Min(8),
        Constraint::Length(if area.height >= 27 { 7 } else { 5 }),
    ])
    .spacing(1)
    .split(area);
    let report = app.shown_report();
    let live = &app.live;
    let samples = app.shown_samples();
    let latest = samples.last();
    let heartbeat = app
        .shown_config()
        .is_some_and(|c| matches!(c.behavior, crate::config::Behavior::PingPong { .. }));
    let active = if app.viewing.is_some() {
        report.map(|r| r.peak_active).unwrap_or(0)
    } else {
        live.active
    };
    let (tx, rx) = if let Some(report) = report {
        if heartbeat {
            (
                report.pings_sent as f64 / report.load_duration_seconds.max(0.000_001),
                report.pongs_received as f64 / report.load_duration_seconds.max(0.000_001),
            )
        } else {
            (
                report.send_messages_per_second,
                report.receive_messages_per_second,
            )
        }
    } else {
        latest
            .map(|s| {
                if heartbeat {
                    (s.ping_tx, s.pong_rx)
                } else {
                    (s.tx, s.rx)
                }
            })
            .unwrap_or((0., 0.))
    };
    let rtt = if heartbeat {
        if let Some(report) = report {
            report.ping_rtt.as_ref()
        } else {
            live.ping_rtt.as_ref()
        }
    } else {
        if let Some(report) = report {
            report.handshake.as_ref()
        } else {
            live.handshake.as_ref()
        }
    };
    let errors: u64 = report
        .map(|r| &r.errors)
        .unwrap_or(&live.errors)
        .values()
        .sum();
    let cards = Layout::horizontal([Constraint::Ratio(1, 4); 4])
        .spacing(1)
        .split(sections[0]);
    let active_series: Vec<u64> = samples.iter().map(|s| s.active as u64).collect();
    let traffic_series: Vec<u64> = samples
        .iter()
        .map(|s| {
            if heartbeat {
                s.ping_tx as u64
            } else {
                s.tx as u64
            }
        })
        .collect();
    metric(
        frame,
        cards[0],
        if app.viewing.is_some() {
            "PEAK CONNECTIONS"
        } else {
            "ACTIVE CONNECTIONS"
        },
        &count(active),
        &format!(
            "{} established · {} peak",
            report.map(|r| r.connected).unwrap_or(live.connected),
            report.map(|r| r.peak_active).unwrap_or(live.peak_active)
        ),
        GREEN,
        &active_series,
    );
    metric(
        frame,
        cards[1],
        if report.is_some() {
            "TX / RX · RUN AVG / SEC"
        } else if heartbeat {
            "PING / PONG · FRAMES / SEC"
        } else {
            "TX / RX · MESSAGES / SEC"
        },
        &format!("{} / {}", rate(tx), rate(rx)),
        if heartbeat {
            "Control frames · matching RTT"
        } else {
            "Application messages"
        },
        CYAN,
        &traffic_series,
    );
    metric(
        frame,
        cards[2],
        if heartbeat {
            "PING RTT · P50"
        } else {
            "HANDSHAKE · P50"
        },
        &latency(rtt.map(|r| r.p50_ms)),
        &format!("p99 {}", latency(rtt.map(|r| r.p99_ms))),
        PURPLE,
        &samples
            .iter()
            .map(|s| (s.rtt.unwrap_or(0.) * 100.) as u64)
            .collect::<Vec<_>>(),
    );
    metric(
        frame,
        cards[3],
        "ERROR EVENTS",
        &count(errors),
        if errors == 0 {
            "No recorded failures"
        } else {
            "Inspect diagnostics below"
        },
        if errors == 0 { GREEN } else { RED },
        &[],
    );
    let plan = app.shown_config().map(LoadPlan::new);
    let total = plan
        .as_ref()
        .map(|plan| plan.duration.as_secs_f64())
        .unwrap_or_else(|| {
            humantime::parse_duration(&app.draft.duration)
                .map(|d| d.as_secs_f64())
                .unwrap_or(30.)
        });
    let elapsed = report
        .map(|r| r.load_duration_seconds)
        .unwrap_or(live.elapsed_seconds);
    frame.render_widget(
        Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(if report.is_some() { GREEN } else { CYAN })
                    .bg(EDGE),
            )
            .ratio((elapsed / total).clamp(0., 1.))
            .label(format!(
                "{} / {}  ·  {}",
                duration(elapsed),
                duration(total),
                if app.viewing.is_some() {
                    "ARCHIVED".into()
                } else if app.phase == Phase::Running {
                    plan.as_ref()
                        .and_then(|plan| {
                            plan.phases
                                .iter()
                                .find(|phase| elapsed < phase.end().as_secs_f64())
                        })
                        .map(|phase| format!("{} · {} target", phase.kind, phase.target))
                        .unwrap_or_else(|| app.phase.name().into())
                } else {
                    app.phase.name().into()
                }
            ))
            .style(Style::default().fg(TEXT)),
        Rect {
            height: 1,
            ..sections[1]
        },
    );
    let graph_cols = Layout::horizontal([
        Constraint::Percentage(if area.width >= 100 { 73 } else { 100 }),
        Constraint::Min(0),
    ])
    .spacing(if area.width >= 100 { 1 } else { 0 })
    .split(sections[2]);
    let graph_rows = Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .spacing(1)
        .split(graph_cols[0]);
    traffic_chart(frame, graph_rows[0], &samples, heartbeat);
    connection_chart(frame, graph_rows[1], &samples, plan.as_ref());
    if graph_cols[1].width > 20 {
        diagnostics(frame, app, graph_cols[1]);
    }
    let events = app
        .logs
        .iter()
        .rev()
        .take(sections[3].height.saturating_sub(2) as usize)
        .collect::<Vec<_>>();
    let lines = if app.viewing.is_some() {
        report
            .map(|report| {
                if report.errors.is_empty() {
                    vec![Line::from(Span::styled(
                        "  Session retained with no recorded failures.",
                        Style::default().fg(GREEN),
                    ))]
                } else {
                    report
                        .errors
                        .iter()
                        .map(|(kind, count)| {
                            Line::from(vec![
                                Span::styled("  ERROR  ", bold(RED)),
                                Span::raw(format!("{kind}  /  {count}")),
                            ])
                        })
                        .collect()
                }
            })
            .unwrap_or_default()
    } else if events.is_empty() {
        vec![Line::from(Span::styled(
            "  No mission launched. Configure a target and press F5.",
            Style::default().fg(MUTED),
        ))]
    } else {
        events
            .into_iter()
            .rev()
            .map(|event| {
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", duration(event.time)),
                        Style::default().fg(MUTED),
                    ),
                    Span::styled(
                        &event.text,
                        Style::default().fg(if event.warning { RED } else { TEXT }),
                    ),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(lines).block(panel(title("03", "EVENT STREAM"))),
        sections[3],
    );
}
fn metric(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    subtitle: &str,
    accent: Color,
    spark: &[u64],
) {
    let block = panel(Line::from(Span::styled(
        format!(" {label} "),
        Style::default().fg(MUTED),
    )));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(value.to_string()).style(bold(accent)),
        Rect { height: 1, ..inner },
    );
    if inner.height > 1 {
        frame.render_widget(
            Paragraph::new(subtitle.to_string()).style(Style::default().fg(MUTED)),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
    if inner.height > 2 && !spark.is_empty() {
        frame.render_widget(
            Sparkline::default()
                .data(spark)
                .style(Style::default().fg(accent)),
            Rect::new(inner.x, inner.y + 2, inner.width, 1),
        );
    }
}
fn axes(samples: &[Sample]) -> (f64, f64) {
    let end = samples.last().map(|s| s.time).unwrap_or(30.).max(1.);
    let start = samples.first().map(|s| s.time).unwrap_or(0.);
    (start, end.max(start + 1.))
}
fn traffic_chart(frame: &mut Frame, area: Rect, samples: &[Sample], heartbeat: bool) {
    let tx: Vec<_> = samples
        .iter()
        .map(|s| (s.time, if heartbeat { s.ping_tx } else { s.tx }))
        .collect();
    let rx: Vec<_> = samples
        .iter()
        .map(|s| (s.time, if heartbeat { s.pong_rx } else { s.rx }))
        .collect();
    let max = tx
        .iter()
        .chain(rx.iter())
        .map(|(_, v)| *v)
        .fold(1., f64::max)
        * 1.15;
    let (start, end) = axes(samples);
    frame.render_widget(
        Chart::new(vec![
            Dataset::default()
                .name("TX")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(CYAN))
                .data(&tx),
            Dataset::default()
                .name("RX")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(GREEN))
                .data(&rx),
        ])
        .legend_position(None)
        .block(panel(Line::from(vec![
            Span::styled(
                if heartbeat {
                    " 01 HEARTBEAT FLOW "
                } else {
                    " 01 MESSAGE FLOW "
                },
                bold(TEXT),
            ),
            Span::styled(" TX ", bold(CYAN)),
            Span::styled(" RX ", bold(GREEN)),
        ])))
        .x_axis(
            Axis::default()
                .style(Style::default().fg(EDGE))
                .bounds([start, end])
                .labels([
                    Span::styled(format!("{}s", start as u64), Style::default().fg(MUTED)),
                    Span::styled(format!("{}s", end as u64), Style::default().fg(MUTED)),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(EDGE))
                .bounds([0., max])
                .labels([
                    Span::styled("0", Style::default().fg(MUTED)),
                    Span::styled(rate(max), Style::default().fg(MUTED)),
                ]),
        ),
        area,
    );
    if samples.is_empty() {
        empty_signal(frame, area, "AWAITING APPLICATION TRAFFIC");
    }
}
fn connection_chart(frame: &mut Frame, area: Rect, samples: &[Sample], plan: Option<&LoadPlan>) {
    let active: Vec<_> = samples.iter().map(|s| (s.time, s.active)).collect();
    let (mut start, end) = axes(samples);
    if samples.len() < 240 {
        start = 0.;
    }
    let target = plan.map_or(100, |plan| plan.connections);
    let mut target_line = Vec::new();
    if let Some(plan) = plan.filter(|plan| !plan.phases.is_empty()) {
        for phase in &plan.phases {
            let phase_start = phase.start.as_secs_f64();
            let phase_end = phase.end().as_secs_f64();
            let left = start.max(phase_start);
            let right = end.min(phase_end);
            if left <= right {
                for time in [left, right] {
                    let fraction = if phase.kind == PhaseKind::Ramp {
                        (time - phase_start) / phase.duration.as_secs_f64()
                    } else {
                        1.
                    };
                    target_line.push((time, fraction * f64::from(phase.target)));
                }
            }
        }
    } else {
        target_line.extend([(start, f64::from(target)), (end, f64::from(target))]);
    }
    let max = samples
        .iter()
        .map(|s| s.active)
        .fold(target.max(1) as f64, f64::max)
        * 1.1;
    frame.render_widget(
        Chart::new(vec![
            Dataset::default()
                .name("TARGET")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(CYAN))
                .data(&target_line),
            Dataset::default()
                .name("ACTIVE")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(GREEN))
                .data(&active),
        ])
        .legend_position(None)
        .block(panel(Line::from(vec![
            Span::styled(" 02 CONNECTION ENVELOPE ", bold(TEXT)),
            Span::styled(" ACTIVE ", bold(GREEN)),
            Span::styled(" TARGET ", Style::default().fg(CYAN)),
        ])))
        .x_axis(
            Axis::default()
                .style(Style::default().fg(EDGE))
                .bounds([start, end])
                .labels([
                    Span::styled(format!("{}s", start as u64), Style::default().fg(MUTED)),
                    Span::styled(format!("{}s", end as u64), Style::default().fg(MUTED)),
                ]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(EDGE))
                .bounds([0., max])
                .labels([
                    Span::styled("0", Style::default().fg(MUTED)),
                    Span::styled(count(max as u64), Style::default().fg(MUTED)),
                ]),
        ),
        area,
    );
    if samples.is_empty() {
        empty_signal(frame, area, "ENGINE STANDBY");
    }
}
fn empty_signal(frame: &mut Frame, area: Rect, text: &str) {
    if area.width > 30 && area.height > 4 {
        frame.render_widget(
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .style(Style::default().fg(MUTED)),
            Rect::new(
                area.x + 8,
                area.y + area.height / 2,
                area.width.saturating_sub(10),
                1,
            ),
        );
    }
}
fn diagnostics(frame: &mut Frame, app: &State, area: Rect) {
    let report = app.shown_report();
    let live = &app.live;
    let config = app.shown_config();
    let handshake = if let Some(report) = report {
        report.handshake.as_ref()
    } else {
        live.handshake.as_ref()
    };
    let sent = report
        .map(|r| r.messages_sent)
        .unwrap_or(live.messages_sent);
    let received = report
        .map(|r| r.messages_received)
        .unwrap_or(live.messages_received);
    let bytes = report
        .map(|r| (r.bytes_sent, r.bytes_received))
        .unwrap_or((live.bytes_sent, live.bytes_received));
    let target = report
        .map(|r| r.target.clone())
        .or_else(|| {
            config.map(|c| {
                let mut u = url::Url::parse(&c.url).unwrap();
                u.set_query(None);
                u.to_string()
            })
        })
        .unwrap_or_else(|| app.draft.url.clone());
    let lines = vec![
        Line::from(Span::styled("TARGET", bold(MUTED))),
        Line::from(Span::styled(target, Style::default().fg(CYAN))),
        Line::from(""),
        Line::from(Span::styled("TRANSPORT", bold(MUTED))),
        Line::from(if config.is_some_and(|c| c.url.starts_with("wss:")) {
            "TLS · VERIFIED"
        } else {
            "WEBSOCKET · PLAIN"
        }),
        Line::from(""),
        pair("TX messages", count(sent)),
        pair("RX messages", count(received)),
        pair("TX payload", bytes_label(bytes.0)),
        pair("RX payload", bytes_label(bytes.1)),
        Line::from(""),
        pair(
            "Ping sent",
            count(report.map(|r| r.pings_sent).unwrap_or(live.pings_sent)),
        ),
        pair(
            "Pong received",
            count(
                report
                    .map(|r| r.pongs_received)
                    .unwrap_or(live.pongs_received),
            ),
        ),
        pair(
            "Skipped slots",
            count(
                report
                    .map(|r| r.missed_send_slots)
                    .unwrap_or(live.missed_send_slots),
            ),
        ),
        Line::from(""),
        Line::from(Span::styled("HANDSHAKE", bold(MUTED))),
        pair("p50", latency(handshake.map(|h| h.p50_ms))),
        pair("p99", latency(handshake.map(|h| h.p99_ms))),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel(" SIGNAL DETAILS ")),
        area,
    );
}
fn pair(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<14}"), Style::default().fg(MUTED)),
        Span::styled(value, Style::default().fg(TEXT)),
    ])
}
fn bytes_label(n: u64) -> String {
    if n >= 1_048_576 {
        format!("{:.1} MiB", n as f64 / 1_048_576.)
    } else {
        format!("{:.1} KiB", n as f64 / 1024.)
    }
}
fn sessions(frame: &mut Frame, app: &State, area: Rect) {
    if app.history.is_empty() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("NO MISSIONS IN THE LOGBOOK", bold(CYAN))),
                Line::from(""),
                Line::from("Run a mission to keep its results and telemetry here."),
                Line::from("The last 20 sessions remain available while the app is open."),
                Line::from("Export JSON to keep a report after closing wsGator."),
                Line::from(""),
                Line::from(Span::styled(
                    "[ 1 ] CONFIGURE YOUR FIRST MISSION",
                    bold(GREEN),
                )),
            ])
            .alignment(Alignment::Center)
            .block(panel(title("03", "SESSION LOGBOOK"))),
            area,
        );
        return;
    }
    let sections = Layout::vertical([Constraint::Min(6), Constraint::Length(3)]).split(area);
    let rows = app.history.iter().map(|session| {
        let r = &session.report;
        let status = if r.interrupted {
            "STOPPED"
        } else if r.has_errors() {
            "ERRORS"
        } else {
            "COMPLETE"
        };
        Row::new(vec![
            Cell::from(format!("#{:03}", session.id)),
            Cell::from(r.behavior.clone()),
            Cell::from(status).style(Style::default().fg(if r.has_errors() {
                RED
            } else if r.interrupted {
                AMBER
            } else {
                GREEN
            })),
            Cell::from(format!("{}/{}", r.connected, r.requested_connections)),
            Cell::from(count(r.messages_sent)),
            Cell::from(count(r.errors.values().sum())),
            Cell::from(duration(r.load_duration_seconds)),
        ])
        .height(2)
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(7),
            Constraint::Min(13),
            Constraint::Length(11),
            Constraint::Length(14),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new([
            "ID",
            "BEHAVIOR",
            "STATUS",
            "CONNECTED",
            "TX MSG",
            "ERRORS",
            "TIME",
        ])
        .style(bold(MUTED))
        .height(2),
    )
    .row_highlight_style(Style::default().bg(EDGE).fg(TEXT))
    .highlight_symbol("▸ ")
    .block(panel(title("03", "SESSION LOGBOOK")));
    frame.render_stateful_widget(
        table,
        sections[0],
        &mut TableState::default().with_selected(Some(app.history_selection)),
    );
    let selected = app
        .history
        .get(app.history_selection)
        .map(|s| s.report.target.as_str())
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(selected, Style::default().fg(CYAN))),
            Line::from(Span::styled(
                "↑ ↓ select  ·  Enter inspect telemetry  ·  F5 rerun  ·  e export JSON",
                Style::default().fg(MUTED),
            )),
        ]),
        sections[1],
    );
}
fn footer(frame: &mut Frame, app: &State, area: Rect) {
    let edit = app.edit.is_some();
    let shortcuts = if edit {
        " Enter save field   Esc cancel   Ctrl+U clear   Tab next field "
    } else {
        " F5 run / rerun   d demo   x stop   e export   1/2/3 screens   ? help   q quit "
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(shortcuts, Style::default().fg(TEXT))),
            Line::from(Span::styled(
                format!(" {}", app.notice),
                Style::default().fg(if app.notice.starts_with("error:") {
                    RED
                } else {
                    MUTED
                }),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(EDGE)),
        ),
        area,
    );
}
fn help(frame: &mut Frame, area: Rect) {
    let width = 66.min(area.width.saturating_sub(4));
    let height = 23.min(area.height.saturating_sub(2));
    let rect = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, rect);
    let lines = vec![
        Line::from(Span::styled("MISSION CONTROL / KEYMAP", bold(CYAN))),
        Line::from(""),
        Line::from("1 / 2 / 3       Configure / Telemetry / Sessions"),
        Line::from("Tab / ↑ / ↓     Focus a setting or select a session"),
        Line::from("← / → / m       Switch load behavior"),
        Line::from("Enter           Edit a setting / inspect a session"),
        Line::from("Ctrl+U          Clear the field being edited"),
        Line::from("Esc             Cancel editing / close this overlay"),
        Line::from("a               Show advanced connection settings"),
        Line::from("F5 / r          Launch or rerun the selected mission"),
        Line::from("d               Try a real local demo (no server setup)"),
        Line::from("x / F6          Stop and drain active clients"),
        Line::from("e               Export selected results as JSON"),
        Line::from("q / Ctrl+C      Drain clients and exit"),
        Line::from(""),
        Line::from("Mouse: click tabs, behavior cards, and settings."),
        Line::from("Profiles restore settings at the next launch."),
        Line::from("Headers, message text and query tokens are not saved."),
        Line::from(""),
        Line::from(Span::styled(" ? / Esc  CLOSE", bold(GREEN))),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(TEXT).bg(PANEL))
            .block(panel(" wsGator / HELP ")),
        rect,
    );
}
