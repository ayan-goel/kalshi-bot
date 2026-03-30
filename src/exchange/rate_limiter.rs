use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

/// Token-bucket rate limiter with separate read/write budgets.
#[derive(Clone, Debug)]
pub struct RateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
}

#[derive(Debug)]
struct RateLimiterInner {
    read_tokens: f64,
    write_tokens: f64,
    max_read: f64,
    max_write: f64,
    read_per_sec: f64,
    write_per_sec: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a rate limiter for the Basic API tier (20 reads/s, 10 writes/s).
    pub fn basic_tier() -> Self {
        Self::new(20.0, 10.0)
    }

    pub fn new(read_per_sec: f64, write_per_sec: f64) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                read_tokens: read_per_sec,
                write_tokens: write_per_sec,
                max_read: read_per_sec,
                max_write: write_per_sec,
                read_per_sec,
                write_per_sec,
                last_refill: Instant::now(),
            })),
        }
    }

    pub async fn acquire_read(&self) {
        self.acquire(true).await;
    }

    pub async fn acquire_write(&self, cost: f64) {
        self.acquire_with_cost(false, cost).await;
    }

    async fn acquire(&self, is_read: bool) {
        self.acquire_with_cost(is_read, 1.0).await;
    }

    async fn acquire_with_cost(&self, is_read: bool, cost: f64) {
        // Bug 3: cap cost at the bucket max so it can always be satisfied.
        // Without this, a cost > max_tokens would spin forever.
        let cost = {
            let inner = self.inner.lock().await;
            let max = if is_read { inner.max_read } else { inner.max_write };
            cost.min(max)
        };

        loop {
            {
                let mut inner = self.inner.lock().await;
                inner.refill();

                let available = if is_read {
                    inner.read_tokens
                } else {
                    inner.write_tokens
                };

                if available >= cost {
                    if is_read {
                        inner.read_tokens -= cost;
                    } else {
                        inner.write_tokens -= cost;
                    }
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl RateLimiterInner {
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;

        self.read_tokens = (self.read_tokens + elapsed * self.read_per_sec).min(self.max_read);
        self.write_tokens = (self.write_tokens + elapsed * self.write_per_sec).min(self.max_write);
    }
}
