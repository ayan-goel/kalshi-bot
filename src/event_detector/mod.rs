use rust_decimal::Decimal;
use std::collections::HashMap;
use tokio::time::Instant;
use tracing::debug;

use crate::config::StrategyConfig;
use crate::orderbook::OrderBook;
use crate::types::MarketTicker;

/// Tracks recent mid-price snapshots for book velocity detection.
#[derive(Debug, Clone)]
struct MidHistory {
    snapshots: Vec<(Instant, Decimal)>,
}

impl MidHistory {
    fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    fn push(&mut self, now: Instant, mid: Decimal) {
        self.snapshots.push((now, mid));
        // Keep only last 60 seconds of data
        let cutoff = now - std::time::Duration::from_secs(60);
        self.snapshots.retain(|(t, _)| *t >= cutoff);
    }

    fn velocity_5s(&self, now: Instant) -> Option<Decimal> {
        let cutoff = now - std::time::Duration::from_secs(5);
        let recent: Vec<_> = self
            .snapshots
            .iter()
            .filter(|(t, _)| *t >= cutoff)
            .collect();
        if recent.len() < 2 {
            return None;
        }
        let first = recent.first()?.1;
        let last = recent.last()?.1;
        Some((last - first).abs())
    }
}

/// Tracks per-market "event" state (auto-widening after sudden moves).
#[derive(Debug, Clone)]
struct EventState {
    active: bool,
    triggered_at: Option<Instant>,
}

/// Event-driven repricing detector.
///
/// Monitors book mid-price velocity and triggers temporary spread widening
/// when a sudden move or taker sweep is detected.
pub struct EventDetector {
    event_threshold: Decimal,
    event_half_spread_multiplier: Decimal,
    event_decay_seconds: u64,
    mid_history: HashMap<MarketTicker, MidHistory>,
    event_states: HashMap<MarketTicker, EventState>,
}

impl EventDetector {
    pub fn new(config: &StrategyConfig) -> Self {
        Self {
            event_threshold: config.event_threshold,
            event_half_spread_multiplier: config.event_half_spread_multiplier,
            event_decay_seconds: config.event_decay_seconds,
            mid_history: HashMap::new(),
            event_states: HashMap::new(),
        }
    }

    /// Record the current book state and check for events.
    /// Call this on every tick.
    pub fn update(&mut self, ticker: &MarketTicker, book: &OrderBook) {
        let now = Instant::now();

        let mid = match book.mid() {
            Some(m) => m,
            None => return,
        };

        let history = self
            .mid_history
            .entry(ticker.clone())
            .or_insert_with(MidHistory::new);
        history.push(now, mid);

        let velocity = history.velocity_5s(now);

        if let Some(vel) = velocity {
            if vel >= self.event_threshold {
                let state = self
                    .event_states
                    .entry(ticker.clone())
                    .or_insert(EventState {
                        active: false,
                        triggered_at: None,
                    });

                if !state.active {
                    debug!(
                        market = %ticker,
                        velocity = %vel,
                        threshold = %self.event_threshold,
                        "Event detected: rapid book movement"
                    );
                    state.active = true;
                    state.triggered_at = Some(now);
                }
            }
        }

        // Expire old events
        if let Some(state) = self.event_states.get_mut(ticker) {
            if state.active {
                if let Some(triggered) = state.triggered_at {
                    if triggered.elapsed().as_secs() > self.event_decay_seconds {
                        debug!(market = %ticker, "Event expired, resuming normal quoting");
                        state.active = false;
                        state.triggered_at = None;
                    }
                }
            }
        }
    }

    /// Returns the spread multiplier for a market.
    /// 1.0 means normal, > 1.0 means event-widened.
    pub fn spread_multiplier(&self, ticker: &MarketTicker) -> Decimal {
        match self.event_states.get(ticker) {
            Some(state) if state.active => {
                // Decay: linearly from event_half_spread_multiplier to 1.0
                if let Some(triggered) = state.triggered_at {
                    let elapsed = triggered.elapsed().as_secs_f64();
                    let total = self.event_decay_seconds as f64;
                    let progress = (elapsed / total).min(1.0);
                    let mult_f = self
                        .event_half_spread_multiplier
                        .to_string()
                        .parse::<f64>()
                        .unwrap_or(3.0);
                    let current = mult_f - (mult_f - 1.0) * progress;
                    Decimal::from_f64_retain(current).unwrap_or(Decimal::ONE)
                } else {
                    self.event_half_spread_multiplier
                }
            }
            _ => Decimal::ONE,
        }
    }

    /// Whether any market is currently in an event state.
    pub fn any_active(&self) -> bool {
        self.event_states.values().any(|s| s.active)
    }

    pub fn clear(&mut self) {
        self.mid_history.clear();
        self.event_states.clear();
    }
}
