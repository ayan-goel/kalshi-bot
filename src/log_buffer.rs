use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

#[derive(Debug, Clone, Serialize)]
pub struct RawLogEntry {
    pub id: u64,
    pub ts: DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: JsonValue,
}

#[derive(Clone)]
pub struct LogBuffer {
    inner: Arc<Mutex<LogBufferInner>>,
}

struct LogBufferInner {
    cap: usize,
    next_id: u64,
    entries: VecDeque<RawLogEntry>,
}

impl LogBuffer {
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(1);
        Self {
            inner: Arc::new(Mutex::new(LogBufferInner {
                cap,
                next_id: 1,
                entries: VecDeque::with_capacity(cap),
            })),
        }
    }

    pub fn from_env(default_cap: usize) -> Self {
        let cap = std::env::var("RAW_LOG_BUFFER_CAP")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(default_cap);
        Self::new(cap)
    }

    pub fn push(
        &self,
        level: &str,
        target: &str,
        message: String,
        fields: JsonMap<String, JsonValue>,
    ) {
        let mut inner = self.inner.lock().expect("log buffer mutex poisoned");

        let entry = RawLogEntry {
            id: inner.next_id,
            ts: Utc::now(),
            level: level.to_string(),
            target: target.to_string(),
            message,
            fields: JsonValue::Object(fields),
        };
        inner.next_id += 1;

        if inner.entries.len() >= inner.cap {
            inner.entries.pop_front();
        }
        inner.entries.push_back(entry);
    }

    pub fn latest(&self, limit: usize) -> Vec<RawLogEntry> {
        let inner = self.inner.lock().expect("log buffer mutex poisoned");
        inner
            .entries
            .iter()
            .rev()
            .take(limit.max(1))
            .cloned()
            .collect()
    }

    pub fn before(&self, before_id: u64, limit: usize) -> Vec<RawLogEntry> {
        let inner = self.inner.lock().expect("log buffer mutex poisoned");
        inner
            .entries
            .iter()
            .rev()
            .filter(|entry| entry.id < before_id)
            .take(limit.max(1))
            .cloned()
            .collect()
    }
}

#[derive(Clone)]
pub struct LogBufferLayer {
    buffer: LogBuffer,
}

impl LogBufferLayer {
    pub fn new(buffer: LogBuffer) -> Self {
        Self { buffer }
    }
}

impl<S> Layer<S> for LogBufferLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let md = event.metadata();

        // Only capture our own bot events; skip noisy library internals
        // (hyper, sqlx, tungstenite, tokio_tungstenite, etc.)
        if !md.target().starts_with("kalshi_bot") {
            return;
        }

        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);

        let message = visitor.message.unwrap_or_else(|| md.name().to_string());

        self.buffer
            .push(md.level().as_str(), md.target(), message, visitor.fields);
    }
}

#[derive(Default)]
struct JsonVisitor {
    message: Option<String>,
    fields: JsonMap<String, JsonValue>,
}

impl JsonVisitor {
    fn insert(&mut self, field: &Field, value: JsonValue) {
        let key = field.name().to_string();
        if key == "message" && self.message.is_none() {
            if let Some(msg) = value.as_str() {
                self.message = Some(msg.to_string());
            } else {
                self.message = Some(value.to_string());
            }
        }
        self.fields.insert(key, value);
    }
}

impl Visit for JsonVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.insert(field, JsonValue::String(value.to_string()));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.insert(field, JsonValue::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.insert(field, JsonValue::Number(JsonNumber::from(value)));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.insert(field, JsonValue::Number(JsonNumber::from(value)));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        match JsonNumber::from_f64(value) {
            Some(v) => self.insert(field, JsonValue::Number(v)),
            None => self.insert(field, JsonValue::String(value.to_string())),
        }
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.insert(field, JsonValue::String(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.insert(field, JsonValue::String(format!("{value:?}")));
    }
}
