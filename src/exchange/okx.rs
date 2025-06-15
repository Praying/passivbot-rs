use crate::config::UserConfig;
use crate::types::{LiveConfig, Market, Ticker, Order, Position, OrderBook, ExchangeParams};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64;
use chrono::Utc;

use super::{Exchange, SendSyncError};

#[derive(Deserialize, Debug)]
struct OkxMarket {
    #[serde(rename = "instId")]
    inst_id: String,
    state: String,
    #[serde(rename = "ctType")]
    ct_type: String,
    #[serde(rename = "listTime")]
    list_time: String,
    #[serde(rename = "tickSz")]
    tick_sz: String,
    #[serde(rename = "lotSz")]
    lot_sz: String,
    #[serde(rename = "minSz")]
    min_sz: String,
    #[serde(rename = "ctVal")]
    ct_val: String,
}

#[derive(Deserialize, Debug)]
struct OkxMarketsResponse {
    data: Vec<OkxMarket>,
}

#[derive(Deserialize, Debug)]
struct OkxBalanceDetail {
    #[serde(rename = "cashBal")]
    cash_bal: String,
    ccy: String,
}

#[derive(Deserialize, Debug)]
struct OkxBalanceData {
    details: Vec<OkxBalanceDetail>,
}

#[derive(Deserialize, Debug)]
struct OkxBalanceResponse {
    data: Vec<OkxBalanceData>,
}

#[derive(Deserialize, Debug)]
struct OkxTicker {
    #[serde(rename = "instId")]
    inst_id: String,
    last: String,
}

#[derive(Deserialize, Debug)]
struct OkxTickerResponse {
    data: Vec<OkxTicker>,
}

#[derive(Serialize, Debug)]
struct OkxOrderRequest<'a> {
    #[serde(rename = "instId")]
    inst_id: &'a str,
    #[serde(rename = "tdMode")]
    td_mode: &'a str,
    side: &'a str,
    #[serde(rename = "posSide")]
    pos_side: &'a str,
    #[serde(rename = "ordType")]
    ord_type: &'a str,
    sz: String,
    px: String,
}

#[derive(Deserialize, Debug)]
struct OkxOrderResponseData {
    #[serde(rename = "ordId")]
    ord_id: String,
    #[serde(rename = "sCode")]
    s_code: String,
}

#[derive(Deserialize, Debug)]
struct OkxOrderResponse {
    data: Vec<OkxOrderResponseData>,
}


pub struct Okx {
    pub client: reqwest::Client,
    user_config: UserConfig,
}

impl Clone for Okx {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            user_config: self.user_config.clone(),
        }
    }
}

impl Okx {
    pub fn new(_live_config: &LiveConfig, user_config: &UserConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            user_config: user_config.clone(),
        }
    }

    fn create_auth_headers(&self, method: &str, request_path: &str, body: &str) -> Result<reqwest::header::HeaderMap, SendSyncError> {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        let message = format!("{}{}{}{}", timestamp, method, request_path, body);
        
        type HmacSha256 = Hmac<Sha256>;
        let mut mac = HmacSha256::new_from_slice(self.user_config.secret.as_bytes())?;
        mac.update(message.as_bytes());
        let result = mac.finalize();
        let signature = base64::encode(result.into_bytes());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("OK-ACCESS-KEY", self.user_config.key.parse()?);
        headers.insert("OK-ACCESS-SIGN", signature.parse()?);
        headers.insert("OK-ACCESS-TIMESTAMP", timestamp.parse()?);
        headers.insert("OK-ACCESS-PASSPHRASE", self.user_config.passphrase.parse()?);
        headers.insert(reqwest::header::CONTENT_TYPE, "application/json".parse()?);

        Ok(headers)
    }
}

#[async_trait]
impl Exchange for Okx {
    fn clone_box(&self) -> Box<dyn Exchange> {
        Box::new(self.clone())
    }

    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError> {
        let url = "https://www.okx.com/api/v5/public/instruments?instType=SWAP";
        let response = self.client.get(url).send().await?.text().await?;
        let parsed: OkxMarketsResponse = serde_json::from_str(&response)?;

        let mut markets = HashMap::new();
        for market in parsed.data {
            let symbol = market.inst_id.replace("-SWAP", "");
            markets.insert(
                symbol.clone(),
                Market {
                    symbol: symbol.clone(),
                    active: market.state == "live",
                    swap: true,
                    linear: market.ct_type == "linear",
                    created_at: market.list_time.parse::<i64>()?,
                },
            );
        }
        Ok(markets)
    }

    async fn fetch_tickers(&self, _symbols: &[String]) -> Result<HashMap<String, Ticker>, SendSyncError> {
        unimplemented!()
    }

    async fn fetch_ticker(&self, symbol: &str) -> Result<f64, SendSyncError> {
        let url = format!("https://www.okx.com/api/v5/market/ticker?instId={}-SWAP", symbol);
        let response = self.client.get(&url).send().await?.text().await?;
        let parsed: OkxTickerResponse = serde_json::from_str(&response)?;

        let ticker = parsed.data.first().ok_or("Ticker not found")?;
        Ok(ticker.last.parse::<f64>()?)
    }

    async fn fetch_order_book(&self, _symbol: &str) -> Result<OrderBook, SendSyncError> {
        unimplemented!()
    }

    async fn fetch_balance(&self) -> Result<f64, SendSyncError> {
        let request_path = "/api/v5/account/balance";
        let headers = self.create_auth_headers("GET", request_path, "")?;
        let url = format!("https://www.okx.com{}", request_path);

        let response = self.client.get(&url).headers(headers).send().await?.text().await?;
        let parsed: OkxBalanceResponse = serde_json::from_str(&response)?;

        let balance_data = parsed.data.first().ok_or("No balance data found")?;
        let usdt_balance = balance_data.details
            .iter()
            .find(|d| d.ccy == "USDT")
            .ok_or("USDT balance not found")?;

        Ok(usdt_balance.cash_bal.parse::<f64>()?)
    }

    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError> {
        let request_path = "/api/v5/trade/order";
        let inst_id = format!("{}-SWAP", order.symbol);
        
        let order_req = OkxOrderRequest {
            inst_id: &inst_id,
            td_mode: "cross",
            side: &order.side,
            pos_side: &order.position_side,
            ord_type: &order.time_in_force,
            sz: order.qty.to_string(),
            px: order.price.to_string(),
        };

        let body = serde_json::to_string(&order_req)?;
        let headers = self.create_auth_headers("POST", request_path, &body)?;
        let url = format!("https://www.okx.com{}", request_path);

        let response = self.client.post(&url).headers(headers).body(body).send().await?.text().await?;
        let parsed: OkxOrderResponse = serde_json::from_str(&response)?;

        let order_response = parsed.data.first().ok_or("No order response data")?;
        if order_response.s_code != "0" {
            return Err(format!("Order placement failed with code {}: {}", order_response.s_code, response).into());
        }

        Ok(())
    }

    async fn cancel_order(&mut self, _order_id: &str) -> Result<(), SendSyncError> {
        unimplemented!()
    }

    async fn fetch_position(&self, _symbol: &str) -> Result<Position, SendSyncError> {
        unimplemented!()
    }

    async fn fetch_exchange_params(&self, symbol: &str) -> Result<ExchangeParams, SendSyncError> {
        let url = format!("https://www.okx.com/api/v5/public/instruments?instType=SWAP&instId={}-SWAP", symbol);
        let response = self.client.get(&url).send().await?.text().await?;
        let parsed: OkxMarketsResponse = serde_json::from_str(&response)?;
        
        let market = parsed.data.first().ok_or("Market not found")?;

        Ok(ExchangeParams {
            qty_step: market.lot_sz.parse::<f64>()?,
            price_step: market.tick_sz.parse::<f64>()?,
            min_qty: market.min_sz.parse::<f64>()?,
            min_cost: 0.1, // OKX does not provide min_cost in instruments endpoint
            c_mult: market.ct_val.parse::<f64>()?,
            inverse: market.ct_type == "inverse",
        })
    }
}