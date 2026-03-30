use anyhow::{Context, Result};
use rust_decimal::Decimal;
use tracing::{debug, instrument, warn};

use crate::config::AppConfig;
use crate::exchange::auth::KalshiAuth;
use crate::exchange::models::*;
use crate::exchange::rate_limiter::RateLimiter;
use crate::types::{Balance, Position, MarketTicker};

#[derive(Clone, Debug)]
pub struct KalshiRestClient {
    http: reqwest::Client,
    base_url: String,
    auth: KalshiAuth,
    rate_limiter: RateLimiter,
}

impl KalshiRestClient {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let auth = KalshiAuth::from_config(config)?;

        Ok(Self {
            http: reqwest::Client::new(),
            base_url: config.exchange.rest_base_url.clone(),
            auth,
            rate_limiter: RateLimiter::basic_tier(),
        })
    }

    pub fn auth(&self) -> &KalshiAuth {
        &self.auth
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    async fn get(&self, path: &str) -> Result<reqwest::Response> {
        self.rate_limiter.acquire_read().await;
        let url = format!("{}{}", self.base_url, path);
        let full_path = format!("/trade-api/v2{}", path);
        let headers = self.auth.sign_request("GET", &full_path);

        let resp = self
            .http
            .get(&url)
            .header("KALSHI-ACCESS-KEY", &headers.api_key)
            .header("KALSHI-ACCESS-TIMESTAMP", &headers.timestamp)
            .header("KALSHI-ACCESS-SIGNATURE", &headers.signature)
            .header("Content-Type", "application/json")
            .send()
            .await
            .with_context(|| format!("GET {path} failed"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("GET {path} returned {status}: {body}");
        }

        Ok(resp)
    }

    async fn post<B: serde::Serialize>(&self, path: &str, body: &B) -> Result<reqwest::Response> {
        self.rate_limiter.acquire_write(1.0).await;
        let url = format!("{}{}", self.base_url, path);
        let full_path = format!("/trade-api/v2{}", path);
        let headers = self.auth.sign_request("POST", &full_path);

        let resp = self
            .http
            .post(&url)
            .header("KALSHI-ACCESS-KEY", &headers.api_key)
            .header("KALSHI-ACCESS-TIMESTAMP", &headers.timestamp)
            .header("KALSHI-ACCESS-SIGNATURE", &headers.signature)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {path} failed"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!(
                path = %path,
                status = %status,
                response_body = %body_text,
                "API error response"
            );
            anyhow::bail!("POST {path} returned {status}: {body_text}");
        }

        Ok(resp)
    }

    async fn delete(&self, path: &str) -> Result<reqwest::Response> {
        self.rate_limiter.acquire_write(1.0).await;
        let url = format!("{}{}", self.base_url, path);
        let full_path = format!("/trade-api/v2{}", path);
        let headers = self.auth.sign_request("DELETE", &full_path);

        let resp = self
            .http
            .delete(&url)
            .header("KALSHI-ACCESS-KEY", &headers.api_key)
            .header("KALSHI-ACCESS-TIMESTAMP", &headers.timestamp)
            .header("KALSHI-ACCESS-SIGNATURE", &headers.signature)
            .header("Content-Type", "application/json")
            .send()
            .await
            .with_context(|| format!("DELETE {path} failed"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("DELETE {path} returned {status}: {body}");
        }

        Ok(resp)
    }

    // ── Portfolio endpoints ──

    #[instrument(skip(self))]
    pub async fn get_balance(&self) -> Result<Balance> {
        let resp = self.get("/portfolio/balance").await?;
        let data: BalanceResponse = resp.json().await?;
        Ok(Balance {
            available: Decimal::new(data.balance, 2),
            portfolio_value: Decimal::new(data.portfolio_value, 2),
        })
    }

    #[instrument(skip(self))]
    pub async fn get_positions(&self) -> Result<Vec<Position>> {
        let resp = self.get("/portfolio/positions?limit=200").await?;
        let data: PositionsListResponse = resp.json().await?;
        Ok(data
            .market_positions
            .into_iter()
            .map(|p| {
                let pos: Decimal = p
                    .position_fp
                    .as_deref()
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_else(|| Decimal::from(p.position.unwrap_or(0)));

                let (yes_c, no_c) = if pos >= Decimal::ZERO {
                    (pos, Decimal::ZERO)
                } else {
                    (Decimal::ZERO, pos.abs())
                };

                let realized_pnl = p
                    .realized_pnl_dollars
                    .as_deref()
                    .and_then(|s| s.parse::<Decimal>().ok())
                    .unwrap_or_else(|| Decimal::new(p.realized_pnl.unwrap_or(0), 2));

                Position {
                    market_ticker: MarketTicker::from(p.ticker.as_str()),
                    yes_contracts: yes_c,
                    no_contracts: no_c,
                    avg_yes_price: None,
                    avg_no_price: None,
                    realized_pnl,
                    unrealized_pnl: Decimal::ZERO,
                }
            })
            .collect())
    }

    #[instrument(skip(self))]
    pub async fn get_fills(&self, limit: Option<u32>) -> Result<Vec<FillResponse>> {
        let lim = limit.unwrap_or(100);
        let resp = self.get(&format!("/portfolio/fills?limit={lim}")).await?;
        let data: FillsListResponse = resp.json().await?;
        Ok(data.fills)
    }

    // ── Order endpoints ──

    #[instrument(skip(self))]
    pub async fn get_orders(&self, status: Option<&str>) -> Result<Vec<OrderResponse>> {
        let path = match status {
            Some(s) => format!("/portfolio/orders?status={s}&limit=200"),
            None => "/portfolio/orders?limit=200".to_string(),
        };
        let resp = self.get(&path).await?;
        let data: OrdersListResponse = resp.json().await?;
        Ok(data.orders)
    }

    #[instrument(skip(self))]
    pub async fn create_order(&self, req: &CreateOrderRequest) -> Result<OrderResponse> {
        let req_json = serde_json::to_string(req).unwrap_or_default();
        debug!(ticker = %req.ticker, side = %req.side, request_body = %req_json, "Creating order");
        let resp = self.post("/portfolio/orders", req).await?;
        let data: CreateOrderResponse = resp.json().await?;
        Ok(data.order)
    }

    #[instrument(skip(self))]
    pub async fn cancel_order(&self, order_id: &str) -> Result<OrderResponse> {
        debug!(order_id = %order_id, "Cancelling order");
        let resp = self.delete(&format!("/portfolio/orders/{order_id}")).await?;
        let data: CancelOrderResponse = resp.json().await?;
        Ok(data.order)
    }

    #[instrument(skip(self))]
    pub async fn batch_cancel_orders(&self, order_ids: Vec<String>) -> Result<Vec<OrderResponse>> {
        if order_ids.is_empty() {
            return Ok(vec![]);
        }

        // BatchCancelOrders costs 0.2 per cancel
        let cost = order_ids.len() as f64 * 0.2;
        self.rate_limiter.acquire_write(cost).await;

        let url = format!("{}/portfolio/orders/batched", self.base_url);
        let full_path = "/trade-api/v2/portfolio/orders/batched";
        let headers = self.auth.sign_request("DELETE", full_path);

        let body = BatchCancelRequest {
            orders: order_ids
                .into_iter()
                .map(|id| BatchCancelOrderItem { order_id: id })
                .collect(),
        };

        let resp = self
            .http
            .delete(&url)
            .header("KALSHI-ACCESS-KEY", &headers.api_key)
            .header("KALSHI-ACCESS-TIMESTAMP", &headers.timestamp)
            .header("KALSHI-ACCESS-SIGNATURE", &headers.signature)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("batch cancel failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();
            warn!(status = %status, body = %body_text, "batch cancel error");
            anyhow::bail!("batch cancel returned {status}: {body_text}");
        }

        let data: BatchCancelResponse = resp.json().await?;
        Ok(data
            .orders
            .into_iter()
            .filter_map(|item| item.order)
            .collect())
    }

    // ── Market endpoints ──

    #[instrument(skip(self))]
    pub async fn get_markets(
        &self,
        status: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Vec<MarketResponse>> {
        let mut path = "/markets?".to_string();
        if let Some(s) = status {
            path.push_str(&format!("status={s}&"));
        }
        path.push_str(&format!("limit={}", limit.unwrap_or(100)));
        let resp = self.get(&path).await?;
        let data: MarketsListResponse = resp.json().await?;
        Ok(data.markets)
    }

    #[instrument(skip(self))]
    pub async fn get_orderbook(&self, market_ticker: &str) -> Result<OrderbookDataFp> {
        let resp = self
            .get(&format!("/markets/{market_ticker}/orderbook"))
            .await?;
        let data: OrderbookResponse = resp.json().await?;
        Ok(data.orderbook_fp)
    }
}
