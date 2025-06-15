use log::info;

use crate::manager::Manager;
use chrono::{DateTime, Utc};

// Forager:
// - Periodically fetches all available markets from the exchange.
// - Scores the markets based on volume, unilateralness, and noisiness.
// - Compares the top-scoring markets with the currently running bots.
// - Starts new bots for top-scoring markets that are not yet running.
// - Stops bots that are running on markets that are no longer in the top list.

#[derive(Clone)]
pub struct Forager {
    manager: Manager,
}

impl Forager {
    pub async fn new(manager: Manager) -> Self {
        Self { manager }
    }

    pub async fn run(&self) -> Vec<String> {
        info!("Forager is running");

        let markets = self
            .manager
            .exchange
            .load_markets()
            .await
            .unwrap_or_default();
        let symbols: Vec<String> = markets.keys().cloned().collect();
        let tickers = self
            .manager
            .exchange
            .fetch_tickers(&symbols)
            .await
            .unwrap_or_default();

        let approved_coins = &self.manager.config.live.approved_coins;
        let ignored_coins = &self.manager.config.live.ignored_coins;
        let empty_means_all_approved = self.manager.config.live.empty_means_all_approved;
        let min_vol = self.manager.config.live.min_vol_24h;
        let min_age_days = self.manager.config.live.minimum_coin_age_days;
        let now = Utc::now();

        let eligible_symbols: Vec<String> = markets
            .iter()
            .filter(|(symbol, market)| {
                market.active
                    && market.swap
                    && market.linear
                    && market.symbol.ends_with("USDT")
                    && !ignored_coins.contains(symbol)
                    && (empty_means_all_approved || approved_coins.contains(symbol))
            })
            .filter_map(|(symbol, market)| {
                if let Some(ticker) = tickers.get(symbol) {
                    if let Some(created_at) = DateTime::from_timestamp(market.created_at / 1000, 0)
                    {
                        let age = now - created_at;
                        if ticker.quote_volume >= min_vol
                            && age >= chrono::Duration::days(min_age_days as i64)
                        {
                            return Some(symbol.clone());
                        }
                    }
                }
                None
            })
            .collect();

        // TODO: implement scoring logic

        eligible_symbols
    }
}
