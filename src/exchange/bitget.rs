use crate::config::UserConfig;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use crate::types::{ExchangeParams, LiveConfig, Market, Ticker, Order, Position, OrderBook};
use super::{Exchange, SendSyncError};
use tracing::{info, error};

const BITGET_API_URL: &str = "https://api.bitget.com";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BitgetResponse<T> {
    code: String,
    msg: String,
    data: T,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BitgetMarket {
    symbol: String,
    status: String,
    quote_coin: String,
    min_trade_amount: String,
    taker_fee_rate: String,
    maker_fee_rate: String,
    price_end_step: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BitgetTicker {
    symbol: String,
    best_bid_price: String,
    best_ask_price: String,
    last_price: String,
    quote_volume: String,
}

pub struct Bitget {
    client: reqwest::Client,
    api_key: String,
    api_secret: String,
    passphrase: String,
}

impl Bitget {
    pub fn new(_live_config: &LiveConfig, user_config: &UserConfig) -> Self {
        Bitget {
            client: reqwest::Client::new(),
            api_key: user_config.key.clone(),
            api_secret: user_config.secret.clone(),
            passphrase: user_config.passphrase.clone(),
        }
    }

    fn sign_request(&self, method: &str, request_path: &str, body: &str) -> (String, String) {
        let timestamp = Utc::now().timestamp_millis().to_string();
        let to_sign = format!("{}{}{}{}", timestamp, method, request_path, body);
        let signature = {
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes()).unwrap();
            mac.update(to_sign.as_bytes());
            base64::encode(mac.finalize().into_bytes())
        };
        (timestamp, signature)
    }
}

#[async_trait]
impl Exchange for Bitget {
    fn clone_box(&self) -> Box<dyn Exchange> {
        Box::new(Bitget {
            client: self.client.clone(),
            api_key: self.api_key.clone(),
            api_secret: self.api_secret.clone(),
            passphrase: self.passphrase.clone(),
        })
    }

    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError> {
        info!("Loading markets from Bitget");
        let url = format!("{}/api/mix/v1/market/contracts?productType=umcbl", BITGET_API_URL);
        let response = self.client.get(&url).send().await?.text().await?;
        let bitget_response: BitgetResponse<Vec<BitgetMarket>> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to load markets: {}", bitget_response.msg);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        let markets = bitget_response
            .data
            .into_iter()
            .map(|m| {
                (
                    m.symbol.clone(),
                    Market {
                        symbol: m.symbol,
                        active: m.status == "normal",
                        swap: true,
                        linear: true,
                        created_at: 0, // Bitget does not provide creation date
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
        info!("Fetching tickers from Bitget");
        let url = format!("{}/api/mix/v1/market/tickers?productType=umcbl", BITGET_API_URL);
        let response = self.client.get(&url).send().await?.text().await?;
        let bitget_response: BitgetResponse<Vec<BitgetTicker>> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to fetch tickers: {}", bitget_response.msg);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        let tickers = bitget_response
            .data
            .into_iter()
            .filter(|t| symbols.is_empty() || symbols.contains(&t.symbol))
            .map(|t| {
                (
                    t.symbol.clone(),
                    Ticker {
                        symbol: t.symbol,
                        bid: t.best_bid_price.parse().unwrap_or(0.0),
                        ask: t.best_ask_price.parse().unwrap_or(0.0),
                        last: t.last_price.parse().unwrap_or(0.0),
                        quote_volume: t.quote_volume.parse().unwrap_or(0.0),
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
            Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Ticker data not found in Bitget response")))
        }
    }

    async fn fetch_order_book(&self, symbol: &str) -> Result<OrderBook, SendSyncError> {
        info!("Fetching order book for symbol: {}", symbol);
        let url = format!("{}/api/mix/v1/market/depth?symbol={}&limit=100", BITGET_API_URL, symbol);
        let response = self.client.get(&url).send().await?.text().await?;
        let bitget_response: BitgetResponse<serde_json::Value> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to fetch order book: {}", bitget_response.msg);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        let asks: Vec<[f64; 2]> = serde_json::from_value(bitget_response.data["asks"].clone())?;
        let bids: Vec<[f64; 2]> = serde_json::from_value(bitget_response.data["bids"].clone())?;

        Ok(OrderBook {
            bids,
            asks,
        })
    }

    async fn fetch_balance(&self) -> Result<f64, SendSyncError> {
        info!("Fetching balance from Bitget");
        let request_path = "/api/mix/v1/account/account";
        let params = "symbol=USDT_UMCBL";
        let url = format!("{}{}?{}", BITGET_API_URL, request_path, params);
        let (timestamp, signature) = self.sign_request("GET", request_path, "");

        let response = self.client.get(&url)
            .header("ACCESS-KEY", &self.api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", &self.passphrase)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .text()
            .await?;

        let bitget_response: BitgetResponse<serde_json::Value> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to fetch balance: {}", bitget_response.msg);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        let balance: f64 = bitget_response.data["usdtEquity"].as_str().unwrap_or("0").parse()?;
        Ok(balance)
    }

    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError> {
        info!("Placing order: {:?}", order);
        let request_path = "/api/mix/v1/order/placeOrder";

        let mut order_request = std::collections::HashMap::new();
        order_request.insert("symbol", order.symbol.clone());
        order_request.insert("marginCoin", "USDT".to_string());
        order_request.insert("size", order.qty.to_string());
        order_request.insert("price", order.price.to_string());
        order_request.insert("side", format!("{}_{}", order.side, order.position_side));
        order_request.insert("orderType", "limit".to_string());
        order_request.insert("timeInForceValue", order.time_in_force.clone());

        let payload = serde_json::to_string(&order_request)?;
        let (timestamp, signature) = self.sign_request("POST", request_path, &payload);

        let response = self.client.post(format!("{}{}", BITGET_API_URL, request_path))
            .header("ACCESS-KEY", &self.api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", &self.passphrase)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?
            .text()
            .await?;

        let bitget_response: BitgetResponse<serde_json::Value> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to place order: {}. Response: {}", bitget_response.msg, response);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        Ok(())
    }

    async fn cancel_order(&mut self, order_id: &str) -> Result<(), SendSyncError> {
        info!("Canceling order: {}", order_id);
        let request_path = "/api/mix/v1/order/cancelOrder";

        let mut order_request = std::collections::HashMap::new();
        order_request.insert("symbol", "BTCUSDT_UMCBL".to_string()); // TODO: get from order
        order_request.insert("marginCoin", "USDT".to_string());
        order_request.insert("orderId", order_id.to_string());

        let payload = serde_json::to_string(&order_request)?;
        let (timestamp, signature) = self.sign_request("POST", request_path, &payload);

        let response = self.client.post(format!("{}{}", BITGET_API_URL, request_path))
            .header("ACCESS-KEY", &self.api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", &self.passphrase)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?
            .text()
            .await?;

        let bitget_response: BitgetResponse<serde_json::Value> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to cancel order: {}. Response: {}", bitget_response.msg, response);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        Ok(())
    }

    async fn fetch_position(&self, symbol: &str) -> Result<Position, SendSyncError> {
        info!("Fetching position for symbol: {}", symbol);
        let request_path = "/api/mix/v1/position/singlePosition";
        let params = format!("symbol={}&marginCoin=USDT", symbol);
        let url = format!("{}{}?{}", BITGET_API_URL, request_path, params);
        let (timestamp, signature) = self.sign_request("GET", request_path, "");

        let response = self.client.get(&url)
            .header("ACCESS-KEY", &self.api_key)
            .header("ACCESS-SIGN", &signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", &self.passphrase)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .text()
            .await?;

        let bitget_response: BitgetResponse<Vec<serde_json::Value>> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to fetch position: {}", bitget_response.msg);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        if let Some(position_data) = bitget_response.data.get(0) {
            let size: f64 = position_data["total"].as_str().unwrap_or("0").parse()?;
            let price: f64 = position_data["averageOpenPrice"].as_str().unwrap_or("0").parse()?;
            Ok(Position { size, price })
        } else {
            Ok(Position { size: 0.0, price: 0.0 })
        }
    }
    async fn fetch_exchange_params(&self, symbol: &str) -> Result<ExchangeParams, SendSyncError> {
        info!("Fetching exchange params for symbol: {}", symbol);
        let url = format!("{}/api/mix/v1/market/contracts?productType=umcbl", BITGET_API_URL);
        let response = self.client.get(&url).send().await?.text().await?;
        let bitget_response: BitgetResponse<Vec<BitgetMarket>> = serde_json::from_str(&response)?;

        if bitget_response.code != "0" {
            error!("Failed to fetch exchange params: {}", bitget_response.msg);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, bitget_response.msg)));
        }

        if let Some(market) = bitget_response.data.into_iter().find(|m| m.symbol == symbol) {
            Ok(ExchangeParams {
                qty_step: 0.0, // not available
                price_step: market.price_end_step.parse()?,
                min_qty: market.min_trade_amount.parse()?,
                min_cost: 0.0, // not available
                c_mult: 1.0,   // not available
                inverse: false, // Bitget futures are not inverse
            })
        } else {
            Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, format!("Could not find market info for {}", symbol))))
        }
    }
}