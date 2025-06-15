use crate::analysis;
use crate::types::{Analysis, BotConfig, Market, Ticker, StateParams, GridOrder, TrailingPriceBundle, Order, OrderBook, EMABands};
use crate::grid::{entries, closes, utils};
use crate::exchange::{Exchange, SendSyncError};
use crate::data;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tracing::{info, warn};

pub struct BacktestResult {
    pub final_balance: f64,
    pub analysis: Analysis,
}

pub async fn run_single(config: &BotConfig) -> Result<BacktestResult, SendSyncError> {
    let mut backtester = Backtester::new(config.clone());
    let result = backtester.run().await?;
    Ok(result)
}

pub struct Backtester {
    pub config: BotConfig,
    pub exchange: Box<dyn Exchange>,
    pub markets: HashMap<String, Market>,
    pub tickers: HashMap<String, Ticker>,
    pub now: DateTime<Utc>,
}

impl Backtester {
    pub fn new(config: BotConfig) -> Self {
        let starting_balance = config.backtest.starting_balance;
        Backtester {
            config,
            exchange: Box::new(crate::exchange::simulated::SimulatedExchange::new(starting_balance)),
            markets: HashMap::new(),
            tickers: HashMap::new(),
            now: Utc::now(),
        }
    }

    pub async fn start(&mut self) -> Result<(), SendSyncError> {
        info!("Starting backtest...");
        let result = self.run().await?;
        info!("Backtest finished. Final balance: {}", result.final_balance);
        info!("Performance Analysis:\n{:#?}", result.analysis);
        Ok(())
    }

    async fn run(&mut self) -> Result<BacktestResult, SendSyncError> {
        info!("Backtester is running...");
        let mut equity_curve = Vec::new();
        let mut all_hlcvs = HashMap::new();

        for (exchange_name, symbols) in &self.config.backtest.symbols {
            for symbol in symbols {
                info!("Preparing data for symbol: {} on exchange: {}", symbol, exchange_name);
                let hlcvs = match data::prepare_hlcvs(
                    &self.config,
                    &self.config.live,
                    symbol,
                    Some(&self.config.backtest.start_date),
                    Some(&self.config.backtest.end_date)
                ).await {
                    Ok(hlcvs) => hlcvs,
                    Err(e) => return Err(e),
                };
                all_hlcvs.insert(symbol.clone(), hlcvs);
            }
        }


        // This is a simplified main loop. A real backtest would need to handle time synchronization
        // across different symbols' data. For now, we process one symbol fully, then the next.
        let symbols_to_backtest = self.config.backtest.symbols.clone();
        for (exchange_name, symbols) in &symbols_to_backtest {
             for symbol in symbols {
                info!("Backtesting symbol: {} on exchange: {}", symbol, exchange_name);
                let hlcvs = match all_hlcvs.get(symbol) {
                    Some(hlcvs) => hlcvs,
                    None => {
                        warn!("No HLCV data found for symbol: {}", symbol);
                        continue;
                    }
                };

                let mut ema0 = 0.0;
            let mut ema1 = 0.0;
            let mut trailing_price_bundle = TrailingPriceBundle::default();

            for i in 0..hlcvs.nrows() {
                let row = hlcvs.row(i);
                let close_price = row[4];

                let current_balance = match self.exchange.fetch_balance().await {
                    Ok(balance) => balance,
                    Err(e) => return Err(e),
                };
                equity_curve.push(current_balance);

                if i == 0 {
                    ema0 = close_price;
                    ema1 = close_price;
                } else {
                    ema0 = utils::calc_ema(ema0, close_price, self.config.bot.long.ema_span_0);
                    ema1 = utils::calc_ema(ema1, close_price, self.config.bot.long.ema_span_1);
                }

                let order_book = OrderBook {
                    bids: vec![[close_price, 0.0]],
                    asks: vec![[close_price, 0.0]],
                };

                let balance = match self.exchange.fetch_balance().await {
                    Ok(balance) => balance,
                    Err(e) => return Err(e),
                };
                let position = match self.exchange.fetch_position(symbol).await {
                    Ok(position) => position,
                    Err(e) => return Err(e),
                };
                let exchange_params = match self.exchange.fetch_exchange_params(symbol).await {
                    Ok(params) => params,
                    Err(e) => return Err(e),
                };

                let state_params = StateParams {
                    balance,
                    order_book,
                    ema_bands: EMABands {
                        upper: f64::max(ema0, ema1),
                        lower: f64::min(ema0, ema1),
                    },
                };

                trailing_price_bundle.min_since_open = f64::min(trailing_price_bundle.min_since_open, close_price);
                trailing_price_bundle.max_since_open = f64::max(trailing_price_bundle.max_since_open, close_price);
                if trailing_price_bundle.min_since_open < trailing_price_bundle.max_since_open {
                    trailing_price_bundle.max_since_min = f64::max(trailing_price_bundle.max_since_min, close_price);
                }
                if trailing_price_bundle.max_since_open > trailing_price_bundle.min_since_open {
                    trailing_price_bundle.min_since_max = f64::min(trailing_price_bundle.min_since_max, close_price);
                }

                let (entry_orders_long, entry_orders_short, close_orders_long, close_orders_short) = {
                    let long_cfg = self.config.bot.long.clone();
                    let short_cfg = self.config.bot.short.clone();

                    let entry_orders_long = entries::calc_entries_long(
                        &exchange_params, &state_params, &long_cfg, &position, &trailing_price_bundle
                    );

                    let entry_orders_short = entries::calc_entries_short(
                        &exchange_params, &state_params, &short_cfg, &position, &trailing_price_bundle
                    );

                    let close_orders_long = closes::calc_closes_long(
                        &exchange_params, &state_params, &long_cfg, &position, &trailing_price_bundle
                    );

                    let close_orders_short = closes::calc_closes_short(
                        &exchange_params, &state_params, &short_cfg, &position, &trailing_price_bundle
                    );
                    (entry_orders_long, entry_orders_short, close_orders_long, close_orders_short)
                };

                if let Err(e) = self.place_grid_orders(symbol, entry_orders_long).await {
                    return Err(e);
                }
                if let Err(e) = self.place_grid_orders(symbol, entry_orders_short).await {
                    return Err(e);
                }
                if let Err(e) = self.place_grid_orders(symbol, close_orders_long).await {
                    return Err(e);
                }
                if let Err(e) = self.place_grid_orders(symbol, close_orders_short).await {
                    return Err(e);
                }
            }
            }
        }
        let final_balance = match self.exchange.fetch_balance().await {
            Ok(balance) => balance,
            Err(e) => return Err(e),
        };
        let analysis = analysis::calculate_metrics(&equity_curve);

        Ok(BacktestResult {
            final_balance,
            analysis,
        })
    }
}

impl Backtester {
    async fn place_grid_orders(&mut self, symbol: &str, grid_orders: Vec<GridOrder>) -> Result<(), SendSyncError> {
        for grid_order in grid_orders {
            let order = Order {
                id: "".to_string(), // Will be set by the exchange
                symbol: symbol.to_string(),
                side: if grid_order.qty > 0.0 { "Buy".to_string() } else { "Sell".to_string() },
                position_side: if grid_order.qty > 0.0 { "Long".to_string() } else { "Short".to_string() },
                qty: grid_order.qty.abs(),
                price: grid_order.price,
                reduce_only: false, // This will be determined by other logic later
                custom_id: grid_order.order_type.to_string(),
                time_in_force: "GTC".to_string(),
            };
            match self.exchange.place_order(&order).await {
                Ok(_) => (),
                Err(e) => return Err(e),
            };
        }
        Ok(())
    }
}