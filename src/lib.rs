/// Public library surface for integration tests and external tooling.
///
/// This exposes the pure business-logic modules so tests can import and exercise
/// the real production code rather than re-implementing logic locally.
pub mod bot_state;
pub mod config;
pub mod cross_market;
pub mod db;
pub mod event_detector;
pub mod exchange;
pub mod execution;
pub mod fair_value;
pub mod log_buffer;
pub mod market_scanner;
pub mod orderbook;
pub mod risk;
pub mod state;
pub mod strategy;
pub mod types;
