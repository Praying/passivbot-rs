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

const BYBIT_API_URL: &str = "https://api.bybit.com";

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitMarket {
    symbol: String,
    status: String,
    contract_type: String,
    quote_coin: String,
    #[serde(rename = "createdTime")]
    created_at: String,
    #[serde(rename = "lotSizeFilter")]
    lot_size_filter: LotSizeFilter,
    #[serde(rename = "priceFilter")]
    price_filter: PriceFilter,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct LotSizeFilter {
    qty_step: String,
    min_order_qty: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PriceFilter {
    tick_size: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitMarketResult {
    list: Vec<BybitMarket>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitTicker {
    symbol: String,
    bid1_price: String,
    ask1_price: String,
    last_price: String,
    #[serde(rename = "volume24h")]
    volume_24h: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitTickerResult {
    list: Vec<BybitTicker>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitResponse<T> {
    ret_code: i32,
    ret_msg: String,
    result: T,
}

#[derive(Deserialize, Debug)]
struct BybitOrderBookEntry(String, String);

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitOrderBookResult {
    b: Vec<BybitOrderBookEntry>,
    a: Vec<BybitOrderBookEntry>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitBalanceResult {
    list: Vec<BybitBalance>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitBalance {
    total_wallet_balance: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitPositionResult {
    list: Vec<BybitPosition>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitPosition {
    symbol: String,
    side: String,
    size: String,
    avg_price: String,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitOrderRequest {
    category: String,
    symbol: String,
    side: String,
    order_type: String,
    qty: String,
    price: Option<String>,
    time_in_force: String,
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BybitCancelOrderRequest {
    category: String,
    symbol: String,
    order_id: String,
}

pub struct Bybit {
    client: reqwest::Client,
    api_key: String,
    api_secret: String,
}

impl Bybit {
    pub fn new(_live_config: &LiveConfig, user_config: &UserConfig) -> Self {
        Bybit {
            client: reqwest::Client::new(),
            api_key: user_config.key.clone(),
            api_secret: user_config.secret.clone(),
        }
    }

    fn sign_request(&self, params: &str) -> (String, String) {
        let timestamp = Utc::now().timestamp_millis().to_string();
        let to_sign = format!("{}{}{}", timestamp, self.api_key, params);
        let signature = {
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes()).unwrap();
            mac.update(to_sign.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        (timestamp, signature)
    }

    fn sign_post_request(&self, payload: &str) -> (String, String, String) {
        let timestamp = Utc::now().timestamp_millis().to_string();
        let recv_window = "5000";
        let to_sign = format!("{}{}{}{}", timestamp, self.api_key, recv_window, payload);
        let signature = {
            type HmacSha256 = Hmac<Sha256>;
            let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes()).unwrap();
            mac.update(to_sign.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        (timestamp, recv_window.to_string(), signature)
    }
}

#[async_trait]
impl Exchange for Bybit {
    fn clone_box(&self) -> Box<dyn Exchange> {
        Box::new(Bybit {
            client: self.client.clone(),
            api_key: self.api_key.clone(),
            api_secret: self.api_secret.clone(),
        })
    }

    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError> {
        info!("Loading markets");
        let url = format!(
            "{}/v5/market/instruments-info?category=linear",
            BYBIT_API_URL
        );
        let response = self.client.get(&url).send().await?.text().await?;
        let bybit_response: BybitResponse<BybitMarketResult> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!("Failed to load markets: {}", bybit_response.ret_msg);
            return Err(bybit_response.ret_msg.into());
        }

        let markets = bybit_response
            .result
            .list
            .into_iter()
            .filter_map(|m| match m.created_at.parse() {
                Ok(created_at) => Some((
                    m.symbol.clone(),
                    Market {
                        symbol: m.symbol,
                        active: m.status == "Trading",
                        swap: m.contract_type == "LinearPerpetual",
                        linear: m.contract_type == "LinearPerpetual",
                        created_at,
                    },
                )),
                Err(_) => {
                    warn!("Could not parse created_at for market: {:?}", m);
                    None
                }
            })
            .collect();

        Ok(markets)
    }

    async fn fetch_tickers(
        &self, symbols: &[String],
    ) -> Result<HashMap<String, Ticker>, SendSyncError> {
        info!("Fetching tickers for symbols: {:?}", symbols);
        let url = format!(
            "{}/v5/market/tickers?category=linear&symbol={}",
            BYBIT_API_URL,
            symbols.join(",")
        );
        let response = self.client.get(&url).send().await?.text().await?;
        let bybit_response: BybitResponse<BybitTickerResult> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!("Failed to fetch tickers: {}", bybit_response.ret_msg);
            return Err(bybit_response.ret_msg.into());
        }

        let tickers = bybit_response
            .result
            .list
            .into_iter()
            .filter_map(|t| {
                match (
                    t.bid1_price.parse(),
                    t.ask1_price.parse(),
                    t.last_price.parse(),
                    t.volume_24h.parse(),
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
            Err("Ticker data not found in Bybit response".into())
        }
    }

    async fn fetch_order_book(&self, symbol: &str) -> Result<OrderBook, SendSyncError> {
        info!("Fetching order book for symbol: {}", symbol);
        let url = format!(
            "{}/v5/market/orderbook?category=linear&symbol={}",
            BYBIT_API_URL, symbol
        );
        let response = self.client.get(&url).send().await?.text().await?;
        let bybit_response: BybitResponse<BybitOrderBookResult> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!("Failed to fetch order book: {}", bybit_response.ret_msg);
            return Err(bybit_response.ret_msg.into());
        }

        let bids = bybit_response
            .result
            .b
            .into_iter()
            .filter_map(|e| match (e.0.parse::<f64>(), e.1.parse::<f64>()) {
                (Ok(price), Ok(qty)) => Some([price, qty]),
                _ => {
                    warn!("Could not parse order book entry: {:?}", e);
                    None
                }
            })
            .collect();
        let asks = bybit_response
            .result
            .a
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
        info!("Fetching balance");
        let recv_window = 5000;
        let params = format!("accountType=UNIFIED&recvWindow={}", recv_window);
        let (timestamp, signature) = self.sign_request(&params);
        let url = format!("{}/v5/account/wallet-balance?{}", BYBIT_API_URL, params);

        let response = self
            .client
            .get(&url)
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-SIGN", signature)
            .send()
            .await?
            .text()
            .await?;

        let bybit_response: BybitResponse<BybitBalanceResult> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!("Failed to fetch balance: {}", bybit_response.ret_msg);
            return Err(bybit_response.ret_msg.into());
        }

        if let Some(balance) = bybit_response.result.list.get(0) {
            Ok(balance.total_wallet_balance.parse()?)
        } else {
            error!("Balance data not found in Bybit response");
            Err("Balance data not found in Bybit response".into())
        }
    }

    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError> {
        info!("Placing order: {:?}", order);
        let order_request = BybitOrderRequest {
            category: "linear".to_string(),
            symbol: order.symbol.clone(),
            side: order.side.clone(),
            order_type: "Limit".to_string(),
            qty: order.qty.to_string(),
            price: Some(order.price.to_string()),
            time_in_force: order.time_in_force.clone(),
        };

        let payload = serde_json::to_string(&order_request)?;
        let (timestamp, recv_window, signature) = self.sign_post_request(&payload);

        let response = self
            .client
            .post(format!("{}/v5/order/create", BYBIT_API_URL))
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?
            .text()
            .await?;

        let bybit_response: BybitResponse<serde_json::Value> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!(
                "Failed to place order: {}. Response: {}",
                bybit_response.ret_msg, response
            );
            return Err(bybit_response.ret_msg.into());
        }

        Ok(())
    }

    async fn cancel_order(&mut self, order_id: &str) -> Result<(), SendSyncError> {
        info!("Canceling order: {}", order_id);
        let cancel_request = BybitCancelOrderRequest {
            category: "linear".to_string(),
            symbol: "BTCUSDT".to_string(), // TODO: Get from order
            order_id: order_id.to_string(),
        };

        let payload = serde_json::to_string(&cancel_request)?;
        let (timestamp, recv_window, signature) = self.sign_post_request(&payload);

        let response = self
            .client
            .post(format!("{}/v5/order/cancel", BYBIT_API_URL))
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?
            .text()
            .await?;

        let bybit_response: BybitResponse<serde_json::Value> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!("Failed to cancel order: {}", bybit_response.ret_msg);
            return Err(bybit_response.ret_msg.into());
        }

        Ok(())
    }

    async fn fetch_position(&self, symbol: &str) -> Result<Position, SendSyncError> {
        info!("Fetching position for symbol: {}", symbol);
        let params = format!("category=linear&symbol={}", symbol);
        let (timestamp, signature) = self.sign_request(&params);
        let url = format!("{}/v5/position/list?{}", BYBIT_API_URL, params);

        let response = self
            .client
            .get(&url)
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp)
            .header("X-BAPI-SIGN", signature)
            .send()
            .await?
            .text()
            .await?;

        let bybit_response: BybitResponse<BybitPositionResult> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!("Failed to fetch position: {}", bybit_response.ret_msg);
            return Err(bybit_response.ret_msg.into());
        }

        if let Some(position) = bybit_response.result.list.get(0) {
            Ok(Position {
                size: position.size.parse()?,
                price: position.avg_price.parse()?,
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
        let url = format!(
            "{}/v5/market/instruments-info?category=linear&symbol={}",
            BYBIT_API_URL, symbol
        );
        let response = self.client.get(&url).send().await?.text().await?;
        let bybit_response: BybitResponse<BybitMarketResult> = serde_json::from_str(&response)?;

        if bybit_response.ret_code != 0 {
            error!(
                "Failed to fetch exchange params: {}",
                bybit_response.ret_msg
            );
            return Err(bybit_response.ret_msg.into());
        }

        if let Some(market) = bybit_response.result.list.get(0) {
            Ok(ExchangeParams {
                qty_step: market.lot_size_filter.qty_step.parse()?,
                price_step: market.price_filter.tick_size.parse()?,
                min_qty: market.lot_size_filter.min_order_qty.parse()?,
                min_cost: 0.0, // Not provided by bybit
                c_mult: 1.0,   // Not provided by bybit
                inverse: market.contract_type != "LinearPerpetual",
            })
        } else {
            Err(format!("Could not find market info for {}", symbol).into())
        }
    }
}
