use crate::types::{
    BotConfig, StateParams, GridOrder, TrailingPriceBundle, Order, Position, OrderBook,
    ExchangeParams, EMABands,
};
use crate::grid::{entries, closes};
use crate::exchange::{Exchange, SendSyncError};
use tracing::{info, error};

#[derive(Clone)]
pub struct Manager {
    pub symbol: String,
    pub config: BotConfig,
    pub exchange: Box<dyn Exchange>,

    // State
    position: Position,
    balance: f64,
    order_book: OrderBook,
    exchange_params: ExchangeParams,
    ema_bands: EMABands,
    trailing_price_bundle: TrailingPriceBundle,
}

impl Manager {
    pub fn new(symbol: String, config: BotConfig, exchange: Box<dyn Exchange>) -> Self {
        Self {
            symbol,
            config,
            exchange,
            position: Default::default(),
            balance: 0.0,
            order_book: Default::default(),
            exchange_params: Default::default(),
            ema_bands: Default::default(),
            trailing_price_bundle: Default::default(),
        }
    }

    pub async fn run(&mut self) {
        info!("[{}] Starting manager", self.symbol);
        loop {
            if self.update_state().await.is_err() {
                // error is already logged in update_state
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                continue;
            }

            self.execute_logic().await;

            // Sleep for a configurable duration
            let delay = self.config.live.execution_delay_seconds;
            tokio::time::sleep(tokio::time::Duration::from_secs_f64(delay)).await;
        }
    }

    async fn update_state(&mut self) -> Result<(), SendSyncError> {
        info!("[{}] Updating state", self.symbol);

        let position_fut = self.exchange.fetch_position(&self.symbol);
        let balance_fut = self.exchange.fetch_balance();
        let order_book_fut = self.exchange.fetch_order_book(&self.symbol);
        let exchange_params_fut = self.exchange.fetch_exchange_params(&self.symbol);

        let (position_res, balance_res, order_book_res, exchange_params_res) = tokio::join!(
            position_fut,
            balance_fut,
            order_book_fut,
            exchange_params_fut
        );

        self.position = position_res.map_err(|e| -> SendSyncError {
            error!("[{}] Failed to fetch position: {}", self.symbol, e);
            e
        })?;
        self.balance = balance_res.map_err(|e| -> SendSyncError {
            error!("[{}] Failed to fetch balance: {}", self.symbol, e);
            e
        })?;
        self.order_book = order_book_res.map_err(|e| -> SendSyncError {
            error!("[{}] Failed to fetch order book: {}", self.symbol, e);
            e
        })?;
        self.exchange_params = exchange_params_res.map_err(|e| -> SendSyncError {
            error!("[{}] Failed to fetch exchange params: {}", self.symbol, e);
            e
        })?;

        // TODO: Implement EMA calculations
        self.ema_bands = Default::default();
        // TODO: Implement trailing price logic
        self.trailing_price_bundle = Default::default();

        Ok(())
    }

    async fn execute_logic(&mut self) {
        info!("[{}] Executing logic", self.symbol);

        let state_params = StateParams {
            balance: self.balance,
            order_book: self.order_book.clone(),
            ema_bands: self.ema_bands.clone(),
        };

        let long_cfg = &self.config.bot.long;
        let short_cfg = &self.config.bot.short;

        let mut all_orders = Vec::new();
        all_orders.extend(entries::calc_entries_long(
            &self.exchange_params,
            &state_params,
            long_cfg,
            &self.position,
            &self.trailing_price_bundle,
        ));
        all_orders.extend(entries::calc_entries_short(
            &self.exchange_params,
            &state_params,
            short_cfg,
            &self.position,
            &self.trailing_price_bundle,
        ));
        all_orders.extend(closes::calc_closes_long(
            &self.exchange_params,
            &state_params,
            long_cfg,
            &self.position,
            &self.trailing_price_bundle,
        ));
        all_orders.extend(closes::calc_closes_short(
            &self.exchange_params,
            &state_params,
            short_cfg,
            &self.position,
            &self.trailing_price_bundle,
        ));

        if let Err(e) = self.place_grid_orders(all_orders).await {
            error!("[{}] Failed to place orders: {}", self.symbol, e);
        }
    }

    async fn place_grid_orders(
        &mut self, grid_orders: Vec<GridOrder>,
    ) -> Result<(), SendSyncError> {
        let price_dist_thresh = self.config.live.price_distance_threshold;
        let mid_price = (self.order_book.best_bid() + self.order_book.best_ask()) / 2.0;

        let mut orders_to_place = Vec::new();
        for grid_order in grid_orders {
            if price_dist_thresh > 0.0 {
                let price_dist = (grid_order.price - mid_price).abs() / mid_price;
                if price_dist > price_dist_thresh {
                    info!("[{}] Skipping order due to price distance threshold: dist {:.4}, thresh {:.4}",
                          self.symbol, price_dist, price_dist_thresh);
                    continue;
                }
            }
            orders_to_place.push(grid_order);
        }

        let batch_size = self.config.live.max_n_creations_per_batch as usize;
        for chunk in orders_to_place.chunks(batch_size) {
            let orders: Vec<Order> = chunk
                .iter()
                .map(|grid_order| Order {
                    id: "".to_string(),
                    symbol: self.symbol.clone(),
                    side: if grid_order.qty > 0.0 {
                        "Buy".to_string()
                    } else {
                        "Sell".to_string()
                    },
                    position_side: if grid_order.qty > 0.0 {
                        "Long".to_string()
                    } else {
                        "Short".to_string()
                    },
                    qty: grid_order.qty.abs(),
                    price: grid_order.price,
                    reduce_only: false,
                    custom_id: grid_order.order_type.to_string(),
                    time_in_force: self.config.live.time_in_force.clone(),
                })
                .collect();

            // In a real scenario, we'd use a batch order endpoint if available.
            // For now, we place them sequentially as before.
            for order in &orders {
                if let Err(e) = self.exchange.place_order(order).await {
                    error!("[{}] Failed to place order: {}", self.symbol, e);
                }
            }
        }

        Ok(())
    }
}
