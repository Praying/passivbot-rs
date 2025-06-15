use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use crate::config::UserConfig;
use crate::types::{ExchangeParams, LiveConfig, Market, Ticker, Order, Position, OrderBook};
use super::{Exchange, SendSyncError};
use tracing::{info, error};

const HYPERLIQUID_API_URL: &str = "https://api.hyperliquid.xyz";

#[derive(Deserialize, Debug)]
struct HyperliquidMarket {
    name: String,
    #[serde(rename = "maxLeverage")]
    max_leverage: f64,
    #[serde(rename = "onlyIsolated")]
    only_isolated: bool,
}

#[derive(Deserialize, Debug)]
struct HyperliquidUniverse {
    universe: Vec<HyperliquidMarket>,
}

pub struct Hyperliquid {
    client: reqwest::Client,
    wallet_address: String,
    private_key: String,
}

impl Hyperliquid {
    pub fn new(_live_config: &LiveConfig, user_config: &UserConfig) -> Self {
        Hyperliquid {
            client: reqwest::Client::new(),
            wallet_address: user_config.key.clone(), // Using key for wallet_address
            private_key: user_config.secret.clone(), // Using secret for private_key
        }
    }

    fn sign_exchange_request(&self, action: serde_json::Value) -> Result<String, SendSyncError> {
        // This is a simplified signing process. A proper implementation would use a proper library for this.
        // For the sake of this example, we'll just serialize the action to a string.
        let payload = serde_json::to_string(&action)?;
        Ok(payload)
    }
}

#[async_trait]
impl Exchange for Hyperliquid {
    fn clone_box(&self) -> Box<dyn Exchange> {
        Box::new(Hyperliquid {
            client: self.client.clone(),
            wallet_address: self.wallet_address.clone(),
            private_key: self.private_key.clone(),
        })
    }

    async fn load_markets(&self) -> Result<HashMap<String, Market>, SendSyncError> {
        info!("Loading markets from Hyperliquid");
        let url = format!("{}/info", HYPERLIQUID_API_URL);
        let body = serde_json::json!({ "type": "meta" });
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;
        let markets_response: HyperliquidUniverse = serde_json::from_str(&response)?;

        let markets = markets_response
            .universe
            .into_iter()
            .map(|m| {
                (
                    m.name.clone(),
                    Market {
                        symbol: m.name,
                        active: !m.only_isolated,
                        swap: true,
                        linear: true,
                        created_at: 0, // not provided
                    },
                )
            })
            .collect();

        Ok(markets)
    }

    async fn fetch_tickers(
        &self, symbols: &[String],
    ) -> Result<HashMap<String, Ticker>, SendSyncError> {
        info!("Fetching tickers from Hyperliquid");
        let url = format!("{}/info", HYPERLIQUID_API_URL);
        let body = serde_json::json!({ "type": "allMids" });
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;
        let tickers: HashMap<String, String> = serde_json::from_str(&response)?;

        let tickers = tickers
            .into_iter()
            .filter(|(s, _)| symbols.is_empty() || symbols.contains(s))
            .map(|(s, p)| {
                let price = p.parse().unwrap_or(0.0);
                (
                    s.clone(),
                    Ticker {
                        symbol: s,
                        bid: price,
                        ask: price,
                        last: price,
                        quote_volume: 0.0, // not provided
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
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Ticker data not found in Hyperliquid response",
            )))
        }
    }

    async fn fetch_order_book(&self, symbol: &str) -> Result<OrderBook, SendSyncError> {
        info!("Fetching order book for symbol: {}", symbol);
        let url = format!("{}/info", HYPERLIQUID_API_URL);
        let body =
            serde_json::json!({ "type": "l2Book", "coin": symbol.replace("/USDC:USDC", "") });
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;
        let order_book_result: serde_json::Value = serde_json::from_str(&response)?;

        let mut bids: Vec<[f64; 2]> = Vec::new();
        let mut asks: Vec<[f64; 2]> = Vec::new();

        if let Some(levels) = order_book_result["levels"].as_array() {
            for level_val in levels {
                if let Some(level) = level_val.as_array() {
                    if level.len() == 2 {
                        let price_str = level[0].as_str().unwrap_or("0");
                        let qty_str = level[1].as_str().unwrap_or("0");

                        let price = price_str.parse::<f64>()?;
                        let qty = qty_str.parse::<f64>()?;

                        if qty > 0.0 {
                            bids.push([price, qty]);
                        } else {
                            asks.push([price, qty.abs()]);
                        }
                    }
                }
            }
        }

        Ok(OrderBook { bids, asks })
    }

    async fn fetch_balance(&self) -> Result<f64, SendSyncError> {
        info!("Fetching balance from Hyperliquid");
        let url = format!("{}/info", HYPERLIQUID_API_URL);
        let body = serde_json::json!({ "type": "clearinghouseState", "user": self.wallet_address });
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;
        let balance_response: serde_json::Value = serde_json::from_str(&response)?;

        let margin_summary = &balance_response["marginSummary"];
        let account_value: f64 = margin_summary["accountValue"]
            .as_str()
            .unwrap_or("0")
            .parse()?;

        let mut unrealized_pnl = 0.0;
        if let Some(asset_positions) = balance_response["assetPositions"].as_array() {
            for position in asset_positions {
                let pnl_str = position["position"]["unrealizedPnl"]
                    .as_str()
                    .unwrap_or("0");
                unrealized_pnl += pnl_str.parse::<f64>().unwrap_or(0.0);
            }
        }

        Ok(account_value - unrealized_pnl)
    }

    async fn place_order(&mut self, order: &Order) -> Result<(), SendSyncError> {
        info!("Placing order: {:?}", order);
        let action = serde_json::json!({
            "type": "order",
            "orders": [
                {
                    "coin": order.symbol.replace("/USDC:USDC", ""),
                    "is_buy": order.side == "buy",
                    "sz": order.qty,
                    "limit_px": order.price,
                    "order_type": {"limit": {"tif": order.time_in_force.clone()}},
                    "reduce_only": order.reduce_only
                }
            ],
            "grouping": "na",
        });

        let payload = self.sign_exchange_request(action)?;

        let response = self
            .client
            .post(format!("{}/exchange", HYPERLIQUID_API_URL))
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?
            .text()
            .await?;

        let response_json: serde_json::Value = serde_json::from_str(&response)?;
        if response_json["status"] == "ok" {
            Ok(())
        } else {
            error!("Failed to place order: {}", response);
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                response,
            )))
        }
    }

    async fn cancel_order(&mut self, order_id: &str) -> Result<(), SendSyncError> {
        info!("Canceling order: {}", order_id);
        let (symbol, oid) = order_id.split_at(order_id.find(':').unwrap_or(order_id.len()));
        let action = serde_json::json!({
            "type": "cancel",
            "cancels": [
                {
                    "coin": symbol.replace("/USDC:USDC", ""),
                    "oid": oid.trim_start_matches(':').parse::<u64>()?
                }
            ]
        });

        let payload = self.sign_exchange_request(action)?;

        let response = self
            .client
            .post(format!("{}/exchange", HYPERLIQUID_API_URL))
            .header("Content-Type", "application/json")
            .body(payload)
            .send()
            .await?
            .text()
            .await?;

        let response_json: serde_json::Value = serde_json::from_str(&response)?;
        if response_json["status"] == "ok" {
            Ok(())
        } else {
            error!("Failed to cancel order: {}", response);
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                response,
            )))
        }
    }

    async fn fetch_position(&self, symbol: &str) -> Result<Position, SendSyncError> {
        info!("Fetching position for symbol: {}", symbol);
        let url = format!("{}/info", HYPERLIQUID_API_URL);
        let body = serde_json::json!({ "type": "clearinghouseState", "user": self.wallet_address });
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;
        let state: serde_json::Value = serde_json::from_str(&response)?;

        if let Some(asset_positions) = state["assetPositions"].as_array() {
            let coin = symbol.replace("/USDC:USDC", "");
            for pos in asset_positions {
                if let Some(position_info) = pos.get("position") {
                    if let Some(pos_coin) = position_info["coin"].as_str() {
                        if pos_coin == coin {
                            let size_str = position_info["szi"].as_str().unwrap_or("0");
                            let price_str = position_info["entryPx"].as_str().unwrap_or("0");
                            if price_str.is_empty() {
                                return Ok(Position {
                                    size: 0.0,
                                    price: 0.0,
                                });
                            }
                            let size = size_str.parse::<f64>()?;
                            let price = price_str.parse::<f64>()?;
                            return Ok(Position { size, price });
                        }
                    }
                }
            }
        }

        Ok(Position {
            size: 0.0,
            price: 0.0,
        })
    }
    async fn fetch_exchange_params(&self, symbol: &str) -> Result<ExchangeParams, SendSyncError> {
        info!("Fetching exchange params for symbol: {}", symbol);
        let url = format!("{}/info", HYPERLIQUID_API_URL);
        let body = serde_json::json!({ "type": "meta" });
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .text()
            .await?;
        let meta: serde_json::Value = serde_json::from_str(&response)?;

        if let Some(universe) = meta["universe"].as_array() {
            let coin = symbol.replace("/USDC:USDC", "");
            for market_data in universe {
                if let Some(name) = market_data["name"].as_str() {
                    if name == coin {
                        return Ok(ExchangeParams {
                            qty_step: 0.0,   // Not available
                            price_step: 0.0, // Not available
                            min_qty: 0.0,    // Not available
                            min_cost: 10.1,  // From python implementation
                            c_mult: 1.0,     // Not available
                            inverse: false,  // Hyperliquid is not inverse
                        });
                    }
                }
            }
        }

        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Could not find market info for {}", symbol),
        )))
    }
}
