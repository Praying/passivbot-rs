use async_trait::async_trait;
use std::collections::HashMap;
use crate::types::{Market, Ticker, Order, Position, OrderBook, ExchangeParams};
use super::{Exchange, SendSyncError};
use tracing::info;

#[derive(Clone)]
pub struct SimulatedExchange {
    pub balance: f64,
    pub position: Position,
    pub orders: Vec<Order>,
}

impl SimulatedExchange {
    pub fn new(starting_balance: f64) -> Self {
        Self {
            balance: starting_balance,
            position: Position {
                size: 0.0,
                price: 0.0,
            },
            orders: Vec::new(),
        }
    }
}

#[async_trait]
impl Exchange for SimulatedExchange {
    fn clone_box(&self) -> Box<dyn Exchange> {
        Box::new(self.clone())
    }

    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError> {
        unimplemented!()
    }

    async fn fetch_tickers(&self, _symbols: &[String]) -> Result<HashMap<String, Ticker>, SendSyncError> {
        unimplemented!()
    }

    async fn fetch_ticker(&self, _symbol: &str) -> Result<f64, SendSyncError> {
        unimplemented!()
    }

    async fn fetch_order_book(&self, _symbol: &str) -> Result<OrderBook, SendSyncError> {
        unimplemented!()
    }

    async fn fetch_balance(&self) -> Result<f64, SendSyncError> {
        Ok(self.balance)
    }

    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError> {
        info!("Placing order: {:?}", order);
        let order_cost = order.qty * order.price;
        if self.balance >= order_cost {
            self.balance -= order_cost;
            let mut new_order = order.clone();
            new_order.id = self.orders.len().to_string();
            self.orders.push(new_order);

            let qty = if order.side == "Buy" { order.qty } else { -order.qty };
            let new_size = self.position.size + qty;
            if new_size == 0.0 {
                self.position.price = 0.0;
            } else {
                self.position.price = (self.position.size * self.position.price + qty * order.price) / new_size;
            }
            self.position.size = new_size;
        }
        Ok(())
    }

    async fn cancel_order(&mut self, order_id: &str) -> Result<(), SendSyncError> {
        info!("Canceling order: {}", order_id);
        if let Some(index) = self.orders.iter().position(|o| o.id == order_id) {
            let order = &self.orders[index];
            let order_cost = order.qty * order.price;
            self.balance += order_cost;
            self.orders.remove(index);
        }
        Ok(())
    }

    async fn fetch_position(&self, _symbol: &str) -> Result<Position, SendSyncError> {
        Ok(self.position.clone())
    }

    async fn fetch_exchange_params(&self, _symbol: &str) -> Result<ExchangeParams, SendSyncError> {
        Ok(ExchangeParams {
            qty_step: 0.001,
            price_step: 0.01,
            min_qty: 0.001,
            min_cost: 1.0,
            c_mult: 1.0,
            inverse: false,
        })
    }
}