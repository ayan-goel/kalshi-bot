use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub environment: String,
    pub exchange: ExchangeConfig,
    pub trading: TradingConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    #[serde(default = "default_api_port")]
    pub api_port: u16,
}

fn default_api_port() -> u16 {
    8080
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeConfig {
    pub rest_base_url: String,
    pub ws_url: String,
    pub api_key_env: String,
    pub private_key_path_env: String,
    #[serde(default)]
    pub production: Option<ExchangeEnvConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeEnvConfig {
    pub rest_base_url: String,
    pub ws_url: String,
    pub api_key_env: String,
    pub private_key_env: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradingConfig {
    pub enabled: bool,
    pub markets_allowlist: Vec<String>,
    pub categories_allowlist: Vec<String>,
    pub max_open_orders: u32,
    pub max_markets_active: u32,
    #[serde(default = "default_rescan_interval")]
    pub market_rescan_interval_mins: u32,
    #[serde(default = "default_min_expiry_hours")]
    pub min_time_to_expiry_hours: f64,
    #[serde(default = "default_max_expiry_hours")]
    pub max_time_to_expiry_hours: f64,
    #[serde(default)]
    pub min_volume_24h: f64,
    #[serde(default)]
    pub market_score_weights: MarketScoreWeights,
}

fn default_rescan_interval() -> u32 {
    15
}
fn default_min_expiry_hours() -> f64 {
    2.0
}
fn default_max_expiry_hours() -> f64 {
    168.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarketScoreWeights {
    #[serde(default = "default_w_volume")]
    pub volume: f64,
    #[serde(default = "default_w_spread")]
    pub spread: f64,
    #[serde(default = "default_w_oi")]
    pub open_interest: f64,
    #[serde(default = "default_w_expiry")]
    pub expiry: f64,
    #[serde(default = "default_w_edge")]
    pub edge: f64,
    #[serde(default = "default_w_price")]
    pub price_centrality: f64,
}

fn default_w_volume() -> f64 {
    0.25
}
fn default_w_spread() -> f64 {
    0.20
}
fn default_w_oi() -> f64 {
    0.15
}
fn default_w_expiry() -> f64 {
    0.15
}
fn default_w_edge() -> f64 {
    0.15
}
fn default_w_price() -> f64 {
    0.10
}

impl Default for MarketScoreWeights {
    fn default() -> Self {
        Self {
            volume: default_w_volume(),
            spread: default_w_spread(),
            open_interest: default_w_oi(),
            expiry: default_w_expiry(),
            edge: default_w_edge(),
            price_centrality: default_w_price(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategyConfig {
    pub base_half_spread: Decimal,
    pub min_edge_after_fees: Decimal,
    pub default_order_size: u32,
    pub max_order_size: u32,
    pub min_rest_ms: u64,
    pub repricing_threshold: Decimal,
    pub inventory_skew_coeff: Decimal,
    pub volatility_widen_coeff: Decimal,
    pub tick_interval_ms: u64,
    pub order_imbalance_alpha: Decimal,
    pub trade_sign_alpha: Decimal,
    pub inventory_penalty_k1: Decimal,
    pub inventory_penalty_k3: Decimal,
    #[serde(default = "default_inv_spread_scale")]
    pub inv_spread_scale: Decimal,
    #[serde(default = "default_inv_skew_scale")]
    pub inv_skew_scale: Decimal,
    #[serde(default = "default_vol_baseline_spread")]
    pub vol_baseline_spread: Decimal,
    #[serde(default = "default_expiry_widen_coeff")]
    pub expiry_widen_coeff: Decimal,
    #[serde(default = "default_expiry_widen_threshold_hours")]
    pub expiry_widen_threshold_hours: f64,
    #[serde(default = "default_event_half_spread_mult")]
    pub event_half_spread_multiplier: Decimal,
    #[serde(default = "default_event_threshold")]
    pub event_threshold: Decimal,
    #[serde(default = "default_event_decay_secs")]
    pub event_decay_seconds: u64,
}

fn default_inv_spread_scale() -> Decimal {
    Decimal::new(1, 1)
} // 0.1
fn default_inv_skew_scale() -> Decimal {
    Decimal::new(1, 2)
} // 0.01
fn default_vol_baseline_spread() -> Decimal {
    Decimal::new(2, 2)
} // 0.02
fn default_expiry_widen_coeff() -> Decimal {
    Decimal::new(1, 2)
} // 0.01
fn default_expiry_widen_threshold_hours() -> f64 {
    4.0
}
fn default_event_half_spread_mult() -> Decimal {
    Decimal::new(3, 0)
} // 3x
fn default_event_threshold() -> Decimal {
    Decimal::new(5, 2)
} // 0.05
fn default_event_decay_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskConfig {
    pub max_loss_daily: Decimal,
    pub max_market_notional: Decimal,
    pub max_market_inventory_contracts: i64,
    pub max_total_reserved: Decimal,
    pub max_open_orders: u32,
    pub cancel_all_on_disconnect: bool,
    /// How long to wait after a disconnect before triggering the kill switch.
    /// Currently informational — the WS reconnect loop retries immediately with
    /// exponential backoff; a future enhancement could honour this timeout.
    pub disconnect_timeout_secs: u64,
    /// Maximum seconds to tolerate a sequence gap before forcing a book resync.
    /// Currently the resync is triggered immediately on any gap; this field
    /// is reserved for a future debounce window.
    pub seq_gap_timeout_secs: u64,
    #[serde(default = "default_max_capital_per_market")]
    pub max_capital_per_market: Decimal,
    #[serde(default = "default_max_portfolio_utilization")]
    pub max_portfolio_utilization: Decimal,
    #[serde(default = "default_max_fair_deviation")]
    pub max_fair_deviation: Decimal,
}

fn default_max_capital_per_market() -> Decimal {
    Decimal::new(1000, 2)
} // $10
fn default_max_portfolio_utilization() -> Decimal {
    Decimal::new(50, 2)
} // 0.50
fn default_max_fair_deviation() -> Decimal {
    Decimal::new(10, 2)
} // 0.10

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url_env: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub json: bool,
}

impl AppConfig {
    pub fn load(config_path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config from {}", config_path.display()))?;
        let config: AppConfig =
            serde_yaml::from_str(&contents).context("Failed to parse YAML config")?;
        Ok(config)
    }

    pub fn api_key(&self) -> Result<String> {
        std::env::var(&self.exchange.api_key_env)
            .with_context(|| format!("Missing env var {}", self.exchange.api_key_env))
    }

    pub fn private_key_path(&self) -> Result<String> {
        std::env::var(&self.exchange.private_key_path_env)
            .with_context(|| format!("Missing env var {}", self.exchange.private_key_path_env))
    }

    pub fn database_url(&self) -> Result<String> {
        std::env::var(&self.database.url_env)
            .with_context(|| format!("Missing env var {}", self.database.url_env))
    }

    pub fn is_demo(&self) -> bool {
        self.environment == "demo"
    }

    pub fn trading_enabled(&self) -> bool {
        let env_override = std::env::var("TRADING_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(false);
        self.trading.enabled || env_override
    }

    pub fn production_api_key(&self) -> Result<String> {
        let env_var = self
            .exchange
            .production
            .as_ref()
            .map(|p| p.api_key_env.as_str())
            .unwrap_or("KALSHI_PROD_API_KEY");
        std::env::var(env_var).with_context(|| format!("Missing env var {env_var}"))
    }

    pub fn production_private_key_base64(&self) -> Result<String> {
        let env_var = self
            .exchange
            .production
            .as_ref()
            .map(|p| p.private_key_env.as_str())
            .unwrap_or("KALSHI_PROD_PRIVATE_KEY_BASE64");
        std::env::var(env_var).with_context(|| format!("Missing env var {env_var}"))
    }

    pub fn exchange_urls_for_env(&self, env: &str) -> (String, String) {
        if env == "production" {
            if let Some(prod) = &self.exchange.production {
                return (prod.rest_base_url.clone(), prod.ws_url.clone());
            }
        }
        (
            self.exchange.rest_base_url.clone(),
            self.exchange.ws_url.clone(),
        )
    }
}
