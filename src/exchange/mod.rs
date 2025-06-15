pub mod bybit;
pub mod binance;
pub mod bitget;
pub mod gateio;
pub mod hyperliquid;
pub mod okx;
pub mod simulated;

use async_trait::async_trait;
use crate::types::{Market, Ticker, Order, Position, OrderBook, ExchangeParams};
use std::collections::HashMap;

pub type SendSyncError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait Exchange: Send + Sync {
    fn clone_box(&self) -> Box<dyn Exchange>;
    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError>;
    async fn fetch_tickers(&self, symbols: &[String]) -> Result<HashMap<String, Ticker>, SendSyncError>;
    async fn fetch_ticker(&self, symbol: &str) -> Result<f64, SendSyncError>;
    async fn fetch_order_book(&self, symbol: &str) -> Result<OrderBook, SendSyncError>;
    async fn fetch_balance(&self) -> Result<f64, SendSyncError>;
    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError>;
    async fn cancel_order(&mut self, order_id: &str) -> Result<(), SendSyncError>;
    async fn fetch_position(&self, symbol: &str) -> Result<Position, SendSyncError>;
    async fn fetch_exchange_params(&self, symbol: &str) -> Result<ExchangeParams, SendSyncError>;
}

impl Clone for Box<dyn Exchange> {
    fn clone(&self) -> Box<dyn Exchange> {
        self.clone_box()
    }
}
