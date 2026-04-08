use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use tracing::{debug, info};

use crate::config::{MarketScoreWeights, TradingConfig};
use crate::exchange::models::MarketResponse;
use crate::exchange::rest::KalshiRestClient;

const MAKER_FEE_RATE: f64 = 0.0175;

#[derive(Debug, Clone, Serialize)]
pub struct ScoredMarket {
    pub ticker: String,
    pub event_ticker: Option<String>,
    pub score: f64,
    pub volume_24h: f64,
    pub spread: f64,
    pub open_interest: f64,
    pub hours_to_expiry: f64,
    pub mid_price: f64,
    pub fee_adjusted_edge: f64,
    pub reject_reason: Option<String>,
}

pub struct MarketScanner {
    categories_allowlist: Vec<String>,
    min_time_to_expiry_hours: f64,
    max_time_to_expiry_hours: f64,
    min_volume_24h: f64,
    weights: MarketScoreWeights,
}

impl MarketScanner {
    pub fn new(config: &TradingConfig) -> Self {
        Self {
            categories_allowlist: config.categories_allowlist.clone(),
            min_time_to_expiry_hours: config.min_time_to_expiry_hours,
            max_time_to_expiry_hours: config.max_time_to_expiry_hours,
            min_volume_24h: config.min_volume_24h,
            weights: config.market_score_weights.clone(),
        }
    }

    /// Scan all open markets and return scored results alongside the raw market data.
    /// Returns `(scored, raw_markets)` — callers can use raw_markets to build metadata
    /// without a second round-trip to the exchange.
    pub async fn scan(
        &self,
        rest_client: &KalshiRestClient,
    ) -> Result<(Vec<ScoredMarket>, Vec<MarketResponse>)> {
        info!("Scanning all open markets...");
        let markets = rest_client
            .get_all_markets(Some("open"), None, None)
            .await?;
        info!(count = markets.len(), "Fetched open markets for scoring");

        let now = Utc::now();
        let mut scored: Vec<ScoredMarket> = markets
            .iter()
            .map(|m| self.score_market(m, &now))
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let accepted = scored.iter().filter(|s| s.reject_reason.is_none()).count();
        info!(
            total = scored.len(),
            accepted = accepted,
            "Market scan complete"
        );

        Ok((scored, markets))
    }

    /// Select the top N markets that pass filters.
    /// Returns `(selected_tickers, all_scored, raw_market_data)`.
    /// The raw market data is returned so callers can build metadata without a second fetch.
    pub async fn select_markets(
        &self,
        rest_client: &KalshiRestClient,
        max_markets: usize,
        existing_allowlist: &[String],
    ) -> Result<(Vec<String>, Vec<ScoredMarket>, Vec<MarketResponse>)> {
        if !existing_allowlist.is_empty() {
            info!(
                count = existing_allowlist.len(),
                "Using explicit markets_allowlist, fetching metadata"
            );
            let raw_markets = rest_client
                .get_all_markets(Some("open"), None, None)
                .await
                .unwrap_or_default();
            let scored: Vec<ScoredMarket> = existing_allowlist
                .iter()
                .map(|t| ScoredMarket {
                    ticker: t.clone(),
                    event_ticker: None,
                    score: 1.0,
                    volume_24h: 0.0,
                    spread: 0.0,
                    open_interest: 0.0,
                    hours_to_expiry: 0.0,
                    mid_price: 0.0,
                    fee_adjusted_edge: 0.0,
                    reject_reason: None,
                })
                .collect();
            let tickers: Vec<String> = existing_allowlist
                .iter()
                .take(max_markets)
                .cloned()
                .collect();
            return Ok((tickers, scored, raw_markets));
        }

        let (all_scored, raw_markets) = self.scan(rest_client).await?;
        let selected: Vec<String> = all_scored
            .iter()
            .filter(|s| s.reject_reason.is_none())
            .take(max_markets)
            .map(|s| s.ticker.clone())
            .collect();

        info!(
            count = selected.len(),
            max = max_markets,
            "Selected top markets by score"
        );
        for (i, s) in all_scored
            .iter()
            .filter(|s| s.reject_reason.is_none())
            .take(max_markets)
            .enumerate()
        {
            debug!(
                rank = i + 1,
                ticker = %s.ticker,
                score = format!("{:.3}", s.score),
                vol24h = format!("{:.1}", s.volume_24h),
                spread = format!("{:.4}", s.spread),
                oi = format!("{:.1}", s.open_interest),
                hours = format!("{:.1}", s.hours_to_expiry),
                mid = format!("{:.4}", s.mid_price),
                "Selected market"
            );
        }

        Ok((selected, all_scored, raw_markets))
    }

    fn score_market(&self, market: &MarketResponse, now: &DateTime<Utc>) -> ScoredMarket {
        let ticker = market.ticker.clone();
        let event_ticker = market.event_ticker.clone();

        let volume_24h = parse_fp(&market.volume_24h_fp);
        let open_interest = parse_fp(&market.open_interest_fp);
        let yes_bid = parse_fp(&market.yes_bid_dollars);
        let yes_ask = parse_fp(&market.yes_ask_dollars);
        let spread = if yes_ask > 0.0 && yes_bid > 0.0 {
            yes_ask - yes_bid
        } else {
            1.0
        };
        let mid_price = if yes_ask > 0.0 && yes_bid > 0.0 {
            (yes_bid + yes_ask) / 2.0
        } else {
            0.5
        };

        let hours_to_expiry = market
            .latest_expiration_time
            .as_deref()
            .or(market.close_time.as_deref())
            .or(market.expiration_time.as_deref())
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|exp| (exp.with_timezone(&Utc) - *now).num_seconds() as f64 / 3600.0)
            .unwrap_or(0.0);

        let maker_fee_at_mid = MAKER_FEE_RATE * mid_price * (1.0 - mid_price);
        let fee_adjusted_edge = (spread / 2.0) - maker_fee_at_mid;

        let mut base = ScoredMarket {
            ticker,
            event_ticker,
            score: 0.0,
            volume_24h,
            spread,
            open_interest,
            hours_to_expiry,
            mid_price,
            fee_adjusted_edge,
            reject_reason: None,
        };

        if let Some(reason) = self.hard_reject(market, &base) {
            base.reject_reason = Some(reason);
            return base;
        }

        base.score = self.compute_score(&base);
        base
    }

    fn hard_reject(&self, market: &MarketResponse, scored: &ScoredMarket) -> Option<String> {
        if market.market_type.as_deref() == Some("scalar") {
            return Some("scalar market".to_string());
        }

        let status = market.status.as_deref().unwrap_or("");
        if status != "open" && status != "active" {
            return Some(format!("status={status}"));
        }

        if market.is_provisional == Some(true) {
            return Some("provisional".to_string());
        }

        if scored.hours_to_expiry < self.min_time_to_expiry_hours {
            return Some(format!(
                "expiry too soon ({:.1}h < {:.1}h)",
                scored.hours_to_expiry, self.min_time_to_expiry_hours
            ));
        }

        if scored.hours_to_expiry > self.max_time_to_expiry_hours {
            return Some(format!(
                "expiry too far ({:.1}h > {:.1}h)",
                scored.hours_to_expiry, self.max_time_to_expiry_hours
            ));
        }

        if scored.fee_adjusted_edge < 0.0 {
            return Some(format!(
                "no edge after fees (spread={:.4}, edge={:.4})",
                scored.spread, scored.fee_adjusted_edge
            ));
        }

        if scored.volume_24h < self.min_volume_24h && self.min_volume_24h > 0.0 {
            return Some(format!(
                "volume too low ({:.1} < {:.1})",
                scored.volume_24h, self.min_volume_24h
            ));
        }

        if !self.categories_allowlist.is_empty() {
            let cat = market.category.as_deref().unwrap_or("");
            if !self
                .categories_allowlist
                .iter()
                .any(|c| c.eq_ignore_ascii_case(cat))
            {
                return Some(format!("category '{cat}' not in allowlist"));
            }
        }

        // Reject prices extremely close to 0 or 1 (thin edge, fee-dominated)
        if scored.mid_price < 0.05 || scored.mid_price > 0.95 {
            return Some(format!("price too extreme ({:.4})", scored.mid_price));
        }

        // Need both sides of the book to market-make
        if scored.spread >= 1.0 {
            return Some("no two-sided book".to_string());
        }

        None
    }

    fn compute_score(&self, s: &ScoredMarket) -> f64 {
        let w = &self.weights;

        // Volume: log scale normalization
        let vol_score = (1.0 + s.volume_24h).ln() / 10.0;

        // Spread: tighter is better; inverse, capped
        let spread_score = if s.spread > 0.0 {
            (1.0 / (1.0 + s.spread * 20.0)).min(1.0)
        } else {
            0.0
        };

        // OI: log scale
        let oi_score = (1.0 + s.open_interest).ln() / 10.0;

        // Expiry: bell curve peaking at 24-72h
        let expiry_score = expiry_bell(s.hours_to_expiry);

        // Edge: normalized (edge / spread)
        let edge_score = if s.spread > 0.0 {
            (s.fee_adjusted_edge / s.spread).max(0.0).min(1.0)
        } else {
            0.0
        };

        // Price centrality: peaks at 0.50, falls toward 0 and 1
        let price_score = 4.0 * s.mid_price * (1.0 - s.mid_price);

        w.volume * vol_score
            + w.spread * spread_score
            + w.open_interest * oi_score
            + w.expiry * expiry_score
            + w.edge * edge_score
            + w.price_centrality * price_score
    }
}

/// Bell curve peaking between 24-72 hours, dropping off outside.
fn expiry_bell(hours: f64) -> f64 {
    if hours <= 0.0 {
        return 0.0;
    }
    let log_h = hours.ln();
    let ideal_center = 48.0_f64.ln(); // ~3.87
    let sigma = 1.2;
    let z = (log_h - ideal_center) / sigma;
    (-0.5 * z * z).exp()
}

fn parse_fp(s: &Option<String>) -> f64 {
    s.as_deref()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0)
}

/// Compute maker fee for a single contract at a given price.
pub fn maker_fee(price: Decimal) -> Decimal {
    let rate = Decimal::new(175, 4); // 0.0175
    let fee = rate * price * (Decimal::ONE - price);
    // Round up to nearest cent
    (fee * Decimal::new(100, 0)).ceil() / Decimal::new(100, 0)
}

/// Compute maker fee for a given count of contracts.
pub fn maker_fee_total(price: Decimal, count: Decimal) -> Decimal {
    let rate = Decimal::new(175, 4);
    let fee = rate * count * price * (Decimal::ONE - price);
    (fee * Decimal::new(100, 0)).ceil() / Decimal::new(100, 0)
}
