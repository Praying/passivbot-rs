use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use crate::config::UserConfig;
use crate::types::{ExchangeParams, LiveConfig, Market, Ticker, Order, Position, OrderBook};
use super::{Exchange, SendSyncError};
use tracing::{info, error};

const GATEIO_API_URL: &str = "https://api.gateio.ws";

#[derive(Deserialize, Debug)]
struct GateioMarket {
    name: String,
    #[serde(rename = "type")]
    market_type: String,
    quanto_multiplier: String,
    leverage_min: String,
    leverage_max: String,
    maintenance_rate: String,
    mark_type: String,
    trade_status: String,
}

#[derive(Deserialize, Debug)]
struct GateioTicker {
    name: String,
    last: String,
    lowest_ask: String,
    highest_bid: String,
    volume_24h_quote: String,
}

pub struct Gateio {
    client: reqwest::Client,
    api_key: String,
    api_secret: String,
}

impl Gateio {
    pub fn new(_live_config: &LiveConfig, user_config: &UserConfig) -> Self {
        Gateio {
            client: reqwest::Client::new(),
            api_key: user_config.key.clone(),
            api_secret: user_config.secret.clone(),
        }
    }

    fn sign_request(&self, method: &str, uri: &str, query_string: &str, body: &str) -> (String, String) {
        let timestamp = (Utc::now().timestamp_millis() as f64 / 1000.0).to_string();
        let mut hasher = Sha256::new();
        hasher.update(body.as_bytes());
        let hashed_payload = hex::encode(hasher.finalize());
        let to_sign = format!("{}\n{}\n{}\n{}\n{}", method, uri, query_string, hashed_payload, timestamp);
        let signature = {
            type HmacSha512 = Hmac<sha2::Sha512>;
            let mut mac = HmacSha512::new_from_slice(self.api_secret.as_bytes()).unwrap();
            mac.update(to_sign.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        (timestamp, signature)
    }
}

#[async_trait]
impl Exchange for Gateio {
    fn clone_box(&self) -> Box<dyn Exchange> {
        Box::new(Gateio {
            client: self.client.clone(),
            api_key: self.api_key.clone(),
            api_secret: self.api_secret.clone(),
        })
    }

    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError> {
        info!("Loading markets from Gate.io");
        let url = format!("{}/api/v4/futures/usdt/contracts", GATEIO_API_URL);
        let response = self.client.get(&url).send().await?.text().await?;
        let markets: Vec<GateioMarket> = serde_json::from_str(&response)?;

        let markets = markets
            .into_iter()
            .map(|m| {
                (
                    m.name.clone(),
                    Market {
                        symbol: m.name,
                        active: m.trade_status == "trading",
                        swap: m.market_type == "futures",
                        linear: true,
                        created_at: 0, // not provided
                    },
                )
            })
            .collect();

        Ok(markets)
    }

    async fn fetch_tickers(
        &self,
        symbols: &[String],
    ) -> Result<HashMap<String, Ticker>, SendSyncError> {
        info!("Fetching tickers from Gate.io");
        let url = format!("{}/api/v4/futures/usdt/tickers", GATEIO_API_URL);
        let response = self.client.get(&url).send().await?.text().await?;
        let tickers: Vec<GateioTicker> = serde_json::from_str(&response)?;

        let tickers = tickers
            .into_iter()
            .filter(|t| symbols.is_empty() || symbols.contains(&t.name))
            .map(|t| {
                (
                    t.name.clone(),
                    Ticker {
                        symbol: t.name,
                        bid: t.highest_bid.parse().unwrap_or(0.0),
                        ask: t.lowest_ask.parse().unwrap_or(0.0),
                        last: t.last.parse().unwrap_or(0.0),
                        quote_volume: t.volume_24h_quote.parse().unwrap_or(0.0),
                    },
                )
            })
            .collect();

        Ok(tickers)
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<f64, SendSyncError> {
        info!("Fetching ticker for symbol: {}", symbol);
        let tickers = self.fetch_tickers(&[symbol.to_string()]).await?;
        if let Some(ticker) = tickers.get(symbol) {
            Ok(ticker.last)
        } else {
            error!("Ticker data not found for symbol: {}", symbol);
            Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Ticker data not found in Gate.io response")))
        }
    }

    async fn fetch_order_book(&self, symbol: &str) -> Result<OrderBook, SendSyncError> {
        info!("Fetching order book for symbol: {}", symbol);
        let url = format!("{}/api/v4/futures/usdt/order_book?contract={}", GATEIO_API_URL, symbol);
        let response = self.client.get(&url).send().await?.text().await?;
        let order_book_result: serde_json::Value = serde_json::from_str(&response)?;

        let asks: Vec<[f64; 2]> = serde_json::from_value(order_book_result["asks"].clone())?;
        let bids: Vec<[f64; 2]> = serde_json::from_value(order_book_result["bids"].clone())?;

        Ok(OrderBook {
            bids,
            asks,
        })
    }

    async fn fetch_balance(&self) -> Result<f64, SendSyncError> {
        info!("Fetching balance from Gate.io");
        let uri = "/api/v4/futures/usdt/accounts";
        let (timestamp, signature) = self.sign_request("GET", uri, "", "");

        let response = self.client.get(format!("{}{}", GATEIO_API_URL, uri))
            .header("KEY", &self.api_key)
            .header("SIGN", &signature)
            .header("Timestamp", &timestamp)
            .send()
            .await?
            .text()
            .await?;
        
        let account: serde_json::Value = serde_json::from_str(&response)?;
        let balance: f64 = account["total"].as_str().unwrap_or("0").parse()?;
        Ok(balance)
    }

    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError> {
        info!("Placing order: {:?}", order);
        let uri = "/api/v4/futures/usdt/orders";

        let mut order_request = std::collections::HashMap::new();
        order_request.insert("contract", order.symbol.clone());
        order_request.insert("size", (order.qty * if order.side == "buy" { 1.0 } else { -1.0 }).to_string());
        order_request.insert("price", order.price.to_string());
        order_request.insert("tif", order.time_in_force.clone());

        let payload = serde_json::to_string(&order_request)?;
        let (timestamp, signature) = self.sign_request("POST", uri, "", &payload);

        let response = self.client.post(format!("{}{}", GATEIO_API_URL, uri))
            .header("KEY", &self.api_key)
            .header("SIGN", &signature)
            .header("Timestamp", &timestamp)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?
            .text()
            .await?;

        let order_response: serde_json::Value = serde_json::from_str(&response)?;

        if order_response.get("id").is_none() {
            error!("Failed to place order: {}", response);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, response)));
        }

        Ok(())
    }

    async fn cancel_order(&mut self, order_id: &str) -> Result<(), SendSyncError> {
        info!("Canceling order: {}", order_id);
        let uri = format!("/api/v4/futures/usdt/orders/{}", order_id);
        let (timestamp, signature) = self.sign_request("DELETE", &uri, "", "");

        let response = self.client.delete(format!("{}{}", GATEIO_API_URL, uri))
            .header("KEY", &self.api_key)
            .header("SIGN", &signature)
            .header("Timestamp", &timestamp)
            .send()
            .await?
            .text()
            .await?;

        let order_response: serde_json::Value = serde_json::from_str(&response)?;

        if order_response.get("id").is_none() {
            error!("Failed to cancel order: {}", response);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, response)));
        }

        Ok(())
    }

    async fn fetch_position(&self, symbol: &str) -> Result<Position, SendSyncError> {
        info!("Fetching position for symbol: {}", symbol);
        let uri = format!("/api/v4/futures/usdt/positions/{}", symbol);
        let (timestamp, signature) = self.sign_request("GET", &uri, "", "");

        let response = self.client.get(format!("{}{}", GATEIO_API_URL, uri))
            .header("KEY", &self.api_key)
            .header("SIGN", &signature)
            .header("Timestamp", &timestamp)
            .send()
            .await?
            .text()
            .await?;

        let position_response: serde_json::Value = serde_json::from_str(&response)?;

        if position_response.get("contract").is_none() {
            Ok(Position { size: 0.0, price: 0.0 })
        } else {
            let size: f64 = position_response["size"].as_str().unwrap_or("0").parse()?;
            let price: f64 = position_response["entry_price"].as_str().unwrap_or("0").parse()?;
            Ok(Position { size, price })
        }
    }
    async fn fetch_exchange_params(&self, symbol: &str) -> Result<ExchangeParams, SendSyncError> {
        info!("Fetching exchange params for symbol: {}", symbol);
        let url = format!("{}/api/v4/futures/usdt/contracts/{}", GATEIO_API_URL, symbol);
        let response = self.client.get(&url).send().await?.text().await?;
        let market: GateioMarket = serde_json::from_str(&response)?;

        Ok(ExchangeParams {
            qty_step: 1.0, // not available from api
            price_step: 0.0, // not available from api
            min_qty: 1.0, // not available from api
            min_cost: 0.0, // not available from api
            c_mult: market.quanto_multiplier.parse()?,
            inverse: false, // Gate.io USDT futures are linear
        })
    }
}