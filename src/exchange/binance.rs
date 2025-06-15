use crate::config::UserConfig;
use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use crate::types::{ExchangeParams, LiveConfig, Market, Ticker, Order, Position, OrderBook};
use super::{Exchange, SendSyncError};
use tracing::{info, error, warn};

const BINANCE_API_URL: &str = "https://fapi.binance.com";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinanceMarket {
    symbol: String,
    status: String,
    contract_type: String,
    onboard_date: i64,
    filters: Vec<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinanceExchangeInfo {
    symbols: Vec<BinanceMarket>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinanceTicker {
    symbol: String,
    bid_price: String,
    ask_price: String,
    last_price: String,
    quote_volume: String,
}

#[derive(Deserialize, Debug)]
struct BinanceOrderBookEntry(String, String);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinanceOrderBookResult {
    bids: Vec<BinanceOrderBookEntry>,
    asks: Vec<BinanceOrderBookEntry>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinanceBalance {
    asset: String,
    balance: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinancePosition {
    symbol: String,
    position_amt: String,
    entry_price: String,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BinanceOrderRequest {
    symbol: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    quantity: String,
    price: String,
    time_in_force: String,
}

pub struct Binance {
    client: reqwest::Client,
    api_key: String,
    api_secret: String,
}

impl Binance {
    pub fn new(_live_config: &LiveConfig, user_config: &UserConfig) -> Self {
        Binance {
            client: reqwest::Client::new(),
            api_key: user_config.key.clone(),
            api_secret: user_config.secret.clone(),
        }
    }

    fn sign_request(&self, params: &str) -> String {
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes()).unwrap();
        mac.update(params.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

#[async_trait]
impl Exchange for Binance {
    fn clone_box(&self) -> Box<dyn Exchange> {
        Box::new(Binance {
            client: self.client.clone(),
            api_key: self.api_key.clone(),
            api_secret: self.api_secret.clone(),
        })
    }

    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError> {
        info!("Loading markets from Binance");
        let url = format!("{}/fapi/v1/exchangeInfo", BINANCE_API_URL);
        let response = self.client.get(&url).send().await?.text().await?;
        let binance_response: BinanceExchangeInfo = serde_json::from_str(&response)?;

        let markets = binance_response
            .symbols
            .into_iter()
            .map(|m| {
                (
                    m.symbol.clone(),
                    Market {
                        symbol: m.symbol,
                        active: m.status == "TRADING",
                        swap: m.contract_type == "PERPETUAL",
                        linear: true, // Binance futures are linear
                        created_at: m.onboard_date,
                    },
                )
            })
            .collect();
        Ok(markets)
    }

    async fn fetch_tickers(
        &self, symbols: &[String],
    ) -> Result<HashMap<String, Ticker>, SendSyncError> {
        info!("Fetching tickers from Binance");
        let url = if symbols.is_empty() {
            format!("{}/fapi/v1/ticker/24hr", BINANCE_API_URL)
        } else {
            // Binance API for single ticker is different
            // For simplicity, we fetch all and filter
            format!("{}/fapi/v1/ticker/24hr", BINANCE_API_URL)
        };

        let response = self.client.get(&url).send().await?.text().await?;
        let binance_tickers: Vec<BinanceTicker> = serde_json::from_str(&response)?;

        let tickers = binance_tickers
            .into_iter()
            .filter(|t| symbols.is_empty() || symbols.contains(&t.symbol))
            .filter_map(|t| {
                match (
                    t.bid_price.parse(),
                    t.ask_price.parse(),
                    t.last_price.parse(),
                    t.quote_volume.parse(),
                ) {
                    (Ok(bid), Ok(ask), Ok(last), Ok(quote_volume)) => Some((
                        t.symbol.clone(),
                        Ticker {
                            symbol: t.symbol,
                            bid,
                            ask,
                            last,
                            quote_volume,
                        },
                    )),
                    _ => {
                        warn!("Could not parse ticker: {:?}", t);
                        None
                    }
                }
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
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Ticker data not found in Binance response",
            )))
        }
    }

    async fn fetch_order_book(&self, symbol: &str) -> Result<OrderBook, SendSyncError> {
        info!("Fetching order book for symbol: {}", symbol);
        let url = format!(
            "{}/fapi/v1/depth?symbol={}&limit=100",
            BINANCE_API_URL, symbol
        );
        let response = self.client.get(&url).send().await?.text().await?;
        let order_book_result: BinanceOrderBookResult = serde_json::from_str(&response)?;

        let bids = order_book_result
            .bids
            .into_iter()
            .filter_map(|e| match (e.0.parse::<f64>(), e.1.parse::<f64>()) {
                (Ok(price), Ok(qty)) => Some([price, qty]),
                _ => {
                    warn!("Could not parse order book entry: {:?}", e);
                    None
                }
            })
            .collect();

        let asks = order_book_result
            .asks
            .into_iter()
            .filter_map(|e| match (e.0.parse::<f64>(), e.1.parse::<f64>()) {
                (Ok(price), Ok(qty)) => Some([price, qty]),
                _ => {
                    warn!("Could not parse order book entry: {:?}", e);
                    None
                }
            })
            .collect();

        Ok(OrderBook { bids, asks })
    }

    async fn fetch_balance(&self) -> Result<f64, SendSyncError> {
        info!("Fetching balance from Binance");
        let timestamp = Utc::now().timestamp_millis();
        let params = format!("timestamp={}", timestamp);
        let signature = self.sign_request(&params);
        let url = format!(
            "{}/fapi/v2/balance?{}&signature={}",
            BINANCE_API_URL, params, signature
        );

        let response = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?
            .text()
            .await?;

        let binance_balances: Vec<BinanceBalance> = serde_json::from_str(&response)?;

        if let Some(usdt_balance) = binance_balances.iter().find(|b| b.asset == "USDT") {
            Ok(usdt_balance.balance.parse()?)
        } else {
            Ok(0.0)
        }
    }

    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError> {
        info!("Placing order on Binance: {:?}", order);
        let order_request = BinanceOrderRequest {
            symbol: order.symbol.clone(),
            side: order.side.clone(),
            order_type: "LIMIT".to_string(),
            quantity: order.qty.to_string(),
            price: order.price.to_string(),
            time_in_force: order.time_in_force.clone(),
        };

        let mut params = serde_urlencoded::to_string(&order_request)?;
        let timestamp = Utc::now().timestamp_millis();
        params.push_str(&format!("&timestamp={}", timestamp));

        let signature = self.sign_request(&params);
        let url = format!(
            "{}/fapi/v1/order?{}&signature={}",
            BINANCE_API_URL, params, signature
        );

        let response = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?
            .text()
            .await?;

        // TODO: better error handling
        if response.contains("code") {
            error!("Failed to place order: {}", response);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                response,
            )));
        }

        Ok(())
    }

    async fn cancel_order(&mut self, order_id: &str) -> Result<(), SendSyncError> {
        info!("Canceling order on Binance: {}", order_id);

        // aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
        let mut params = format!("symbol=BTCUSDT&orderId={}", order_id); // TODO: get symbol from order
        let timestamp = Utc::now().timestamp_millis();
        params.push_str(&format!("&timestamp={}", timestamp));

        let signature = self.sign_request(&params);
        let url = format!(
            "{}/fapi/v1/order?{}&signature={}",
            BINANCE_API_URL, params, signature
        );

        let response = self
            .client
            .delete(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?
            .text()
            .await?;

        // TODO: better error handling
        if response.contains("code") {
            error!("Failed to cancel order: {}", response);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                response,
            )));
        }

        Ok(())
    }

    async fn fetch_position(&self, symbol: &str) -> Result<Position, SendSyncError> {
        info!("Fetching position for symbol: {}", symbol);
        let timestamp = Utc::now().timestamp_millis();
        let params = format!("symbol={}&timestamp={}", symbol, timestamp);
        let signature = self.sign_request(&params);
        let url = format!(
            "{}/fapi/v2/positionRisk?{}&signature={}",
            BINANCE_API_URL, params, signature
        );

        let response = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await?
            .text()
            .await?;

        let positions: Vec<BinancePosition> = serde_json::from_str(&response)?;

        if let Some(position) = positions.iter().find(|p| p.symbol == symbol) {
            Ok(Position {
                size: position.position_amt.parse()?,
                price: position.entry_price.parse()?,
            })
        } else {
            Ok(Position {
                size: 0.0,
                price: 0.0,
            })
        }
    }
    async fn fetch_exchange_params(&self, symbol: &str) -> Result<ExchangeParams, SendSyncError> {
        info!("Fetching exchange params for symbol: {}", symbol);
        let url = format!("{}/fapi/v1/exchangeInfo", BINANCE_API_URL);
        let response = self.client.get(&url).send().await?.text().await?;
        let exchange_info: BinanceExchangeInfo = serde_json::from_str(&response)?;

        if let Some(market) = exchange_info
            .symbols
            .into_iter()
            .find(|m| m.symbol == symbol)
        {
            let mut qty_step = 0.0;
            let mut price_step = 0.0;
            let mut min_qty = 0.0;
            let mut min_cost = 0.0;

            for filter in market.filters {
                match filter.get("filterType").and_then(|v| v.as_str()) {
                    Some("LOT_SIZE") => {
                        qty_step = filter
                            .get("stepSize")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .parse()?;
                        min_qty = filter
                            .get("minQty")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .parse()?;
                    }
                    Some("PRICE_FILTER") => {
                        price_step = filter
                            .get("tickSize")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .parse()?;
                    }
                    Some("MIN_NOTIONAL") => {
                        min_cost = filter
                            .get("notional")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0")
                            .parse()?;
                    }
                    _ => {}
                }
            }

            Ok(ExchangeParams {
                qty_step,
                price_step,
                min_qty,
                min_cost,
                c_mult: 1.0,    // Not provided by binance
                inverse: false, // Binance futures are not inverse
            })
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Could not find market info for {}", symbol),
            )))
        }
    }
}
