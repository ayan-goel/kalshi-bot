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
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskConfig {
    pub max_loss_daily: Decimal,
    pub max_market_notional: Decimal,
    pub max_market_inventory_contracts: i64,
    pub max_total_reserved: Decimal,
    pub max_open_orders: u32,
    pub cancel_all_on_disconnect: bool,
    pub disconnect_timeout_secs: u64,
    pub seq_gap_timeout_secs: u64,
}

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
