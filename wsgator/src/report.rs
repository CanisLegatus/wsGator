use crate::{PhaseKind, load::Phase};
use hdrhistogram::Histogram;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Default)]
pub(crate) struct Counters {
    pub attempted: AtomicU64,
    pub connected: AtomicU64,
    pub active: AtomicU64,
    pub peak_active: AtomicU64,
    pub completed: AtomicU64,
    pub cancelled_connects: AtomicU64,
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub pings_sent: AtomicU64,
    pub pings_received: AtomicU64,
    pub pongs_received: AtomicU64,
    pub pongs_sent: AtomicU64,
    pub missed_send_slots: AtomicU64,
    pub pending_pings_at_stop: AtomicU64,
    pub errors: Mutex<BTreeMap<String, u64>>,
    pub handshake: Mutex<Option<Histogram<u64>>>,
    pub rtt: Mutex<Option<Histogram<u64>>>,
    pub phase_latency: Option<Mutex<PhaseLatency>>,
}

#[derive(Default)]
pub(crate) struct PhaseLatency {
    handshake: Option<Histogram<u64>>,
    ping_rtt: Option<Histogram<u64>>,
}
pub(crate) fn inc(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn add(counter: &AtomicU64, value: usize) {
    counter.fetch_add(value as u64, Ordering::Relaxed);
}
pub(crate) fn load(counter: &AtomicU64) -> u64 {
    counter.load(Ordering::Relaxed)
}
impl Counters {
    pub fn error(&self, category: impl Into<String>) {
        *self
            .errors
            .lock()
            .unwrap()
            .entry(category.into())
            .or_default() += 1;
    }
    pub fn opened(&self) {
        inc(&self.connected);
        let active = self.active.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak_active.fetch_max(active, Ordering::Relaxed);
    }
    pub fn record_handshake(&self, duration: Duration) {
        Self::record(&mut self.handshake.lock().unwrap(), duration);
        if let Some(phase) = &self.phase_latency {
            Self::record(&mut phase.lock().unwrap().handshake, duration);
        }
    }
    pub fn record_ping_rtt(&self, duration: Duration) {
        Self::record(&mut self.rtt.lock().unwrap(), duration);
        if let Some(phase) = &self.phase_latency {
            Self::record(&mut phase.lock().unwrap().ping_rtt, duration);
        }
    }
    fn record(h: &mut Option<Histogram<u64>>, duration: Duration) {
        let h = h.get_or_insert_with(|| {
            Histogram::new_with_bounds(1, 7 * 24 * 3600 * 1_000_000, 3).unwrap()
        });
        // Microsecond resolution and fixed bounds keep memory independent of message count.
        let value = duration
            .as_micros()
            .clamp(1, (7 * 24 * 3600 * 1_000_000u64) as u128) as u64;
        h.record(value).expect("sample is within histogram bounds");
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Latency {
    pub samples: u64,
    pub min_ms: f64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}
impl Latency {
    pub(crate) fn snapshot(h: &Mutex<Option<Histogram<u64>>>) -> Option<Self> {
        Self::from_histogram(h.lock().unwrap().as_ref())
    }
    fn from_histogram(h: Option<&Histogram<u64>>) -> Option<Self> {
        h.filter(|h| !h.is_empty()).map(|h| Self {
            samples: h.len(),
            min_ms: h.min() as f64 / 1000.,
            mean_ms: h.mean() / 1000.,
            p50_ms: h.value_at_quantile(0.5) as f64 / 1000.,
            p95_ms: h.value_at_quantile(0.95) as f64 / 1000.,
            p99_ms: h.value_at_quantile(0.99) as f64 / 1000.,
            max_ms: h.max() as f64 / 1000.,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PhaseReport {
    pub kind: PhaseKind,
    pub target_connections: u32,
    pub planned_duration_seconds: Option<f64>,
    pub started_at_seconds: f64,
    pub duration_seconds: f64,
    pub active_at_start: u64,
    pub active_at_end: u64,
    pub attempted: u64,
    pub connected: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub pings_sent: u64,
    pub pongs_received: u64,
    pub handshake: Option<Latency>,
    pub ping_rtt: Option<Latency>,
    pub errors: BTreeMap<String, u64>,
}

pub(crate) struct PhaseStart {
    pub kind: PhaseKind,
    pub target: u32,
    pub planned_duration: Option<Duration>,
    pub snapshot: LiveSnapshot,
}

impl PhaseStart {
    pub fn scheduled(phase: Phase, snapshot: LiveSnapshot) -> Self {
        Self {
            kind: phase.kind,
            target: phase.target,
            planned_duration: Some(phase.duration),
            snapshot,
        }
    }

    pub fn finish(self, end: &LiveSnapshot, counters: &Counters) -> PhaseReport {
        let latency =
            std::mem::take(&mut *counters.phase_latency.as_ref().unwrap().lock().unwrap());
        let start = self.snapshot;
        let errors = end.errors.iter().filter_map(|(kind, &count)| {
            let delta = count - start.errors.get(kind).copied().unwrap_or(0);
            (delta > 0).then(|| (kind.clone(), delta))
        });
        PhaseReport {
            kind: self.kind,
            target_connections: self.target,
            planned_duration_seconds: self.planned_duration.map(|d| d.as_secs_f64()),
            started_at_seconds: start.elapsed_seconds,
            duration_seconds: end.elapsed_seconds - start.elapsed_seconds,
            active_at_start: start.active,
            active_at_end: end.active,
            attempted: end.attempted - start.attempted,
            connected: end.connected - start.connected,
            messages_sent: end.messages_sent - start.messages_sent,
            messages_received: end.messages_received - start.messages_received,
            pings_sent: end.pings_sent - start.pings_sent,
            pongs_received: end.pongs_received - start.pongs_received,
            handshake: Latency::from_histogram(latency.handshake.as_ref()),
            ping_rtt: Latency::from_histogram(latency.ping_rtt.as_ref()),
            errors: errors.collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub version: &'static str,
    pub target: String,
    pub behavior: String,
    pub requested_connections: u32,
    pub connect_concurrency: u32,
    pub connect_timeout_seconds: f64,
    pub write_timeout_seconds: f64,
    pub close_timeout_seconds: f64,
    pub max_message_bytes: u32,
    pub subprotocol: Option<String>,
    pub configured_duration_seconds: f64,
    /// Excludes close-handshake drain time; rates use this denominator.
    pub load_duration_seconds: f64,
    pub elapsed_seconds: f64,
    pub interrupted: bool,
    pub payload_bytes: Option<usize>,
    pub messages_per_second_per_connection: Option<u32>,
    pub ping_interval_seconds: Option<f64>,
    pub pong_timeout_seconds: Option<f64>,
    pub attempted: u64,
    pub connected: u64,
    pub peak_active: u64,
    pub active_at_end: u64,
    pub completed: u64,
    pub cancelled_connects: u64,
    pub not_attempted: u64,
    /// Application text/binary messages only; control frames are separate.
    pub messages_sent: u64,
    pub messages_received: u64,
    /// Application payload bytes, excluding WebSocket/TLS/TCP overhead.
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub pings_sent: u64,
    pub pings_received: u64,
    pub pongs_received: u64,
    pub pongs_sent: u64,
    pub pending_pings_at_stop: u64,
    pub missed_send_slots: u64,
    pub send_messages_per_second: f64,
    pub receive_messages_per_second: f64,
    pub handshake: Option<Latency>,
    pub ping_rtt: Option<Latency>,
    pub errors: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<PhaseReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_profile: Option<String>,
}
impl Report {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
    pub fn summary(&self) -> String {
        let mut s = format!(
            "wsGator {} — {}\nConnected: {}/{} | peak: {} | active at end: {}\nLoad: {:.3}s | total with shutdown: {:.3}s{}\nMessages: {} sent / {} received | payload bytes: {} sent / {} received\nThroughput: {:.1} sent/s / {:.1} received/s\nHeartbeat: {} Ping sent / {} Pong received | {} server Ping answered\n",
            self.version,
            self.behavior,
            self.connected,
            self.requested_connections,
            self.peak_active,
            self.active_at_end,
            self.load_duration_seconds,
            self.elapsed_seconds,
            if self.interrupted {
                " (interrupted)"
            } else {
                ""
            },
            self.messages_sent,
            self.messages_received,
            self.bytes_sent,
            self.bytes_received,
            self.send_messages_per_second,
            self.receive_messages_per_second,
            self.pings_sent,
            self.pongs_received,
            self.pongs_sent
        );
        for (label, latency) in [("Handshake", &self.handshake), ("Ping RTT", &self.ping_rtt)] {
            if let Some(l) = latency {
                s.push_str(&format!(
                    "{label}: p50 {:.3}ms | p95 {:.3}ms | p99 {:.3}ms ({} samples)\n",
                    l.p50_ms, l.p95_ms, l.p99_ms, l.samples
                ));
            }
        }
        s.push_str(&format!(
            "Missed send slots: {} | cancelled handshakes: {} | not attempted: {}\n",
            self.missed_send_slots, self.cancelled_connects, self.not_attempted
        ));
        if self.errors.is_empty() {
            s.push_str("Errors: 0\n");
        } else {
            for (kind, count) in &self.errors {
                s.push_str(&format!("Error {kind}: {count}\n"));
            }
        }
        for phase in &self.phases {
            let errors: u64 = phase.errors.values().sum();
            s.push_str(&format!(
                "{} {:.3}s | target {} | active {} → {} | errors {}\n",
                phase.kind,
                phase.duration_seconds,
                phase.target_connections,
                phase.active_at_start,
                phase.active_at_end,
                errors,
            ));
        }
        s
    }
}

/// A bounded, replaceable observation for interactive frontends. Counters are cumulative;
/// consumers compute rates from consecutive samples rather than from rendering frequency.
#[derive(Debug, Clone, Default)]
pub struct LiveSnapshot {
    pub elapsed_seconds: f64,
    pub draining: bool,
    pub finished: bool,
    pub attempted: u64,
    pub connected: u64,
    pub active: u64,
    pub peak_active: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub pings_sent: u64,
    pub pongs_received: u64,
    pub missed_send_slots: u64,
    pub handshake: Option<Latency>,
    pub ping_rtt: Option<Latency>,
    pub errors: BTreeMap<String, u64>,
}
impl Counters {
    pub fn snapshot(&self, elapsed_seconds: f64, draining: bool, finished: bool) -> LiveSnapshot {
        LiveSnapshot {
            elapsed_seconds,
            draining,
            finished,
            attempted: load(&self.attempted),
            connected: load(&self.connected),
            active: load(&self.active),
            peak_active: load(&self.peak_active),
            messages_sent: load(&self.messages_sent),
            messages_received: load(&self.messages_received),
            bytes_sent: load(&self.bytes_sent),
            bytes_received: load(&self.bytes_received),
            pings_sent: load(&self.pings_sent),
            pongs_received: load(&self.pongs_received),
            missed_send_slots: load(&self.missed_send_slots),
            handshake: Latency::snapshot(&self.handshake),
            ping_rtt: Latency::snapshot(&self.rtt),
            errors: self.errors.lock().unwrap().clone(),
        }
    }
}
