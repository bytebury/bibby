//! In-process telemetry for the admin monitoring page (`/admin/monitoring`).
//!
//! Two things live here, both in memory and both per replica (they reset when
//! the process restarts):
//!
//! - **Recent logs.** [`init_tracing`] installs the usual stdout formatter
//!   plus a layer that keeps the last [`LOG_CAPACITY`] events at INFO and
//!   above in a ring buffer, so admins can read them without the Railway
//!   console.
//! - **Request stats.** The [`track_requests`] middleware counts responses by
//!   status class, buckets them per minute for the last hour, and remembers
//!   the latest server errors and slow requests (method + path only; query
//!   strings are dropped so nothing sensitive is kept).

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, filter::LevelFilter};

/// How many log lines the page can show.
pub const LOG_CAPACITY: usize = 1000;
/// How many recent server errors / slow requests are remembered.
const RECENT_CAPACITY: usize = 25;
/// A request slower than this is listed as slow.
const SLOW_REQUEST_MS: u64 = 1000;
/// Minutes of per-minute request counts kept for the traffic chart.
pub const TRAFFIC_MINUTES: usize = 60;

static STARTED_AT: LazyLock<DateTime<Utc>> = LazyLock::new(Utc::now);
static LOGS: LazyLock<Mutex<VecDeque<LogEntry>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(LOG_CAPACITY)));
static LOG_SEQ: AtomicU64 = AtomicU64::new(0);
static REQUESTS: LazyLock<Mutex<RequestStats>> = LazyLock::new(Default::default);

/// When this process started.
pub fn started_at() -> DateTime<Utc> {
    *STARTED_AT
}

/// Install the global tracing subscriber: stdout (as before) plus the
/// in-memory log buffer. Call once, first thing in `main`.
pub fn init_tracing() {
    LazyLock::force(&STARTED_AT);
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(LevelFilter::INFO))
        .with(LogBuffer.with_filter(LevelFilter::INFO))
        .init();
}

// ---------------------------------------------------------------------------
// Logs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub seq: u64,
    pub at: DateTime<Utc>,
    pub level: Level,
    pub target: String,
    pub message: String,
}

impl LogEntry {
    pub fn level_name(&self) -> &'static str {
        match self.level {
            Level::ERROR => "error",
            Level::WARN => "warn",
            Level::INFO => "info",
            Level::DEBUG => "debug",
            Level::TRACE => "trace",
        }
    }
}

/// The lowest level a log view asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
}

impl LogLevel {
    pub fn from_key(key: &str) -> Self {
        match key {
            "error" => Self::Error,
            "warn" => Self::Warn,
            _ => Self::Info,
        }
    }

    fn admits(self, level: Level) -> bool {
        match self {
            Self::Error => level == Level::ERROR,
            Self::Warn => level <= Level::WARN,
            Self::Info => level <= Level::INFO,
        }
    }
}

/// The newest `limit` log lines at `level` or above whose text contains
/// `search` (case-insensitive), newest first.
pub fn recent_logs(level: LogLevel, search: &str, limit: usize) -> Vec<LogEntry> {
    let needle = search.trim().to_lowercase();
    let logs = LOGS.lock().unwrap_or_else(|e| e.into_inner());
    logs.iter()
        .rev()
        .filter(|e| level.admits(e.level))
        .filter(|e| {
            needle.is_empty()
                || e.message.to_lowercase().contains(&needle)
                || e.target.to_lowercase().contains(&needle)
        })
        .take(limit)
        .cloned()
        .collect()
}

/// How many buffered lines are warnings and errors.
pub fn log_counts() -> (usize, usize, usize) {
    let logs = LOGS.lock().unwrap_or_else(|e| e.into_inner());
    let errors = logs.iter().filter(|e| e.level == Level::ERROR).count();
    let warnings = logs.iter().filter(|e| e.level == Level::WARN).count();
    (logs.len(), warnings, errors)
}

struct LogBuffer;

impl<S: Subscriber> Layer<S> for LogBuffer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let entry = LogEntry {
            seq: LOG_SEQ.fetch_add(1, Ordering::Relaxed),
            at: Utc::now(),
            level: *event.metadata().level(),
            target: event.metadata().target().to_string(),
            message: visitor.finish(),
        };
        let mut logs = LOGS.lock().unwrap_or_else(|e| e.into_inner());
        if logs.len() == LOG_CAPACITY {
            logs.pop_front();
        }
        logs.push_back(entry);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
    fields: String,
}

impl MessageVisitor {
    fn finish(self) -> String {
        match (self.message.is_empty(), self.fields.is_empty()) {
            (_, true) => self.message,
            (true, false) => self.fields,
            (false, false) => format!("{} {}", self.message, self.fields),
        }
    }

    fn push_field(&mut self, name: &str, value: &str) {
        if !self.fields.is_empty() {
            self.fields.push(' ');
        }
        let _ = write!(self.fields, "{name}={value}");
    }
}

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        } else {
            self.push_field(field.name(), value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.push_field(field.name(), &format!("{value:?}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct RequestRecord {
    pub at: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub status: u16,
    pub millis: u64,
}

/// One minute of traffic.
#[derive(Clone, Copy, Debug, Default)]
pub struct MinuteBucket {
    /// Minutes since the Unix epoch.
    pub minute: i64,
    pub requests: u64,
    pub errors: u64,
}

#[derive(Clone, Debug, Default)]
pub struct RequestStats {
    pub total: u64,
    pub status_2xx: u64,
    pub status_3xx: u64,
    pub status_4xx: u64,
    pub status_5xx: u64,
    pub total_millis: u64,
    pub max_millis: u64,
    minutes: VecDeque<MinuteBucket>,
    pub recent_errors: VecDeque<RequestRecord>,
    pub recent_slow: VecDeque<RequestRecord>,
}

impl RequestStats {
    pub fn average_millis(&self) -> u64 {
        self.total_millis.checked_div(self.total).unwrap_or(0)
    }

    /// Per-minute counts for the last [`TRAFFIC_MINUTES`] minutes, oldest
    /// first, with quiet minutes filled in as zero.
    pub fn traffic(&self, now: DateTime<Utc>) -> Vec<MinuteBucket> {
        let current = now.timestamp() / 60;
        (0..TRAFFIC_MINUTES as i64)
            .rev()
            .map(|ago| {
                let minute = current - ago;
                self.minutes
                    .iter()
                    .find(|b| b.minute == minute)
                    .copied()
                    .unwrap_or(MinuteBucket {
                        minute,
                        ..Default::default()
                    })
            })
            .collect()
    }

    fn record(&mut self, record: RequestRecord) {
        self.total += 1;
        match record.status {
            200..=299 => self.status_2xx += 1,
            300..=399 => self.status_3xx += 1,
            400..=499 => self.status_4xx += 1,
            _ => self.status_5xx += 1,
        }
        self.total_millis += record.millis;
        self.max_millis = self.max_millis.max(record.millis);

        let minute = record.at.timestamp() / 60;
        let is_error = record.status >= 500;
        match self.minutes.back_mut() {
            Some(bucket) if bucket.minute == minute => {
                bucket.requests += 1;
                bucket.errors += u64::from(is_error);
            }
            _ => self.minutes.push_back(MinuteBucket {
                minute,
                requests: 1,
                errors: u64::from(is_error),
            }),
        }
        while self
            .minutes
            .front()
            .is_some_and(|b| b.minute <= minute - TRAFFIC_MINUTES as i64)
        {
            self.minutes.pop_front();
        }

        if is_error {
            push_capped(&mut self.recent_errors, record.clone());
        }
        if record.millis >= SLOW_REQUEST_MS {
            push_capped(&mut self.recent_slow, record);
        }
    }
}

fn push_capped(list: &mut VecDeque<RequestRecord>, record: RequestRecord) {
    if list.len() == RECENT_CAPACITY {
        list.pop_back();
    }
    list.push_front(record);
}

/// A copy of the request stats since this process started.
pub fn request_stats() -> RequestStats {
    REQUESTS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Middleware recording every response's status and latency. Static assets
/// and the monitoring page's own polling are left out so they don't drown
/// out real traffic.
pub async fn track_requests(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if path.starts_with("/assets/") || path.starts_with("/admin/monitoring") {
        return next.run(request).await;
    }
    let method = request.method().to_string();
    let started = Instant::now();
    let response = next.run(request).await;
    let record = RequestRecord {
        at: Utc::now(),
        method,
        path: truncate(path, 120),
        status: response.status().as_u16(),
        millis: started.elapsed().as_millis() as u64,
    };
    REQUESTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .record(record);
    response
}

fn truncate(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push('…');
    }
    s
}

// ---------------------------------------------------------------------------
// Process
// ---------------------------------------------------------------------------

/// Resident memory and thread count from `/proc/self/status` (Linux only).
pub fn process_usage() -> (Option<u64>, Option<u64>) {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return (None, None);
    };
    let field = |name: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|n| n.parse::<u64>().ok())
    };
    (field("VmRSS:").map(|kb| kb * 1024), field("Threads:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(at: DateTime<Utc>, status: u16, millis: u64) -> RequestRecord {
        RequestRecord {
            at,
            method: "GET".into(),
            path: "/".into(),
            status,
            millis,
        }
    }

    #[test]
    fn counts_requests_by_status_and_minute() {
        let now = Utc::now();
        let mut stats = RequestStats::default();
        stats.record(record(now, 200, 10));
        stats.record(record(now, 404, 20));
        stats.record(record(now, 500, 1500));

        assert_eq!(stats.total, 3);
        assert_eq!(
            (stats.status_2xx, stats.status_4xx, stats.status_5xx),
            (1, 1, 1)
        );
        assert_eq!(stats.average_millis(), 510);
        assert_eq!(stats.recent_errors.len(), 1);
        assert_eq!(stats.recent_slow.len(), 1);

        let traffic = stats.traffic(now);
        assert_eq!(traffic.len(), TRAFFIC_MINUTES);
        let last = traffic.last().unwrap();
        assert_eq!((last.requests, last.errors), (3, 1));
    }

    #[test]
    fn forgets_minutes_older_than_an_hour() {
        let now = Utc::now();
        let mut stats = RequestStats::default();
        stats.record(record(now - chrono::Duration::minutes(90), 200, 1));
        stats.record(record(now, 200, 1));
        assert_eq!(stats.minutes.len(), 1);
        assert_eq!(
            stats.traffic(now).iter().map(|b| b.requests).sum::<u64>(),
            1
        );
    }

    #[test]
    fn level_filter_includes_more_severe_levels() {
        assert!(LogLevel::Warn.admits(Level::ERROR));
        assert!(LogLevel::Warn.admits(Level::WARN));
        assert!(!LogLevel::Warn.admits(Level::INFO));
        assert!(!LogLevel::Error.admits(Level::WARN));
        assert!(LogLevel::Info.admits(Level::INFO));
    }

    #[test]
    fn truncates_on_a_char_boundary() {
        assert_eq!(truncate("héllo".into(), 2), "h…");
        assert_eq!(truncate("short".into(), 10), "short");
    }
}
