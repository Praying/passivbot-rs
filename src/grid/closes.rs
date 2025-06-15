use crate::types::{
    BotSideConfig, ExchangeParams, GridOrder, OrderType, Position, StateParams, TrailingPriceBundle,
};
use tracing::warn;
use super::utils::{
    calc_close_grid_backwards_long, calc_close_grid_backwards_short,
    calc_close_grid_frontwards_long, calc_close_grid_frontwards_short,
};

/// Calculates a trailing close order for a long position.
///
/// This function implements the trailing stop logic for taking profit on a long position.
/// It triggers a close order when the price retraces from a recent high.
///
/// # Returns
///
/// A `Vec<GridOrder>` containing the single trailing close order if conditions are met.
/// An empty Vec otherwise.
fn calc_trailing_close_long(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> Vec<GridOrder> {
    if position.size == 0.0 || bot_params.close_trailing_threshold_pct <= 0.0 {
        return vec![];
    }

    let threshold_price = position.price * (1.0 + bot_params.close_trailing_threshold_pct);
    let retracement_price =
        trailing_price_bundle.max_since_open * (1.0 - bot_params.close_trailing_retracement_pct);

    if trailing_price_bundle.max_since_open > threshold_price
        && state_params.order_book.best_bid() < retracement_price
    {
        // Trailing stop triggered
        let close_qty = position.size * bot_params.close_trailing_qty_pct;
        let close_price = state_params.order_book.best_bid();

        let min_qty = super::entries::calc_min_entry_qty(close_price, exchange_params);

        if close_qty >= min_qty {
            return vec![GridOrder {
                qty: -close_qty,
                price: close_price,
                order_type: OrderType::CloseTrailingLong,
            }];
        }
    }

    vec![]
}

pub fn calc_closes_long(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> Vec<GridOrder> {
    // Basic router: if trailing is enabled, use it. Otherwise, use grid.
    // A more sophisticated router like in entries.rs could be implemented later.
    if bot_params.close_trailing_threshold_pct > 0.0
        && bot_params.close_trailing_retracement_pct > 0.0
    {
        let trailing_closes = calc_trailing_close_long(
            exchange_params,
            state_params,
            bot_params,
            position,
            trailing_price_bundle,
        );
        if !trailing_closes.is_empty() {
            return trailing_closes;
        }
    }

    let closes = if bot_params.backwards_tp {
        calc_close_grid_backwards_long(
            state_params.balance,
            position.size,
            position.price,
            state_params.order_book.best_ask(),
            state_params.ema_bands.upper,
            0.0,
            0.0,
            exchange_params.inverse,
            exchange_params.qty_step,
            exchange_params.price_step,
            exchange_params.min_qty,
            exchange_params.min_cost,
            exchange_params.c_mult,
            bot_params.total_wallet_exposure_limit,
            bot_params.close_grid_min_markup,
            bot_params.close_grid_markup_range,
            bot_params.n_close_orders,
            bot_params.unstuck_threshold,
            bot_params.unstuck_ema_dist,
            0.0, // unstuck_delay_minutes
            0.0, // unstuck_qty_pct
        )
    } else {
        calc_close_grid_frontwards_long(
            state_params.balance,
            position.size,
            position.price,
            state_params.order_book.best_ask(),
            state_params.ema_bands.upper,
            0.0,
            0.0,
            exchange_params.inverse,
            exchange_params.qty_step,
            exchange_params.price_step,
            exchange_params.min_qty,
            exchange_params.min_cost,
            exchange_params.c_mult,
            bot_params.total_wallet_exposure_limit,
            bot_params.close_grid_min_markup,
            bot_params.close_grid_markup_range,
            bot_params.n_close_orders,
            bot_params.unstuck_threshold,
            bot_params.unstuck_ema_dist,
            0.0, // unstuck_delay_minutes
            0.0, // unstuck_qty_pct
        )
    };
    closes
        .iter()
        .filter_map(
            |(qty, price, order_type_str)| match OrderType::from_str(order_type_str) {
                Some(order_type) => Some(GridOrder {
                    qty: *qty,
                    price: *price,
                    order_type,
                }),
                None => {
                    warn!("Unknown order type string: {}", order_type_str);
                    None
                }
            },
        )
        .collect()
}

/// Calculates a trailing close order for a short position.
///
/// This function implements the trailing stop logic for taking profit on a short position.
/// It triggers a close order when the price retraces from a recent low.
///
/// # Returns
///
/// A `Vec<GridOrder>` containing the single trailing close order if conditions are met.
/// An empty Vec otherwise.
fn calc_trailing_close_short(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> Vec<GridOrder> {
    if position.size == 0.0 || bot_params.close_trailing_threshold_pct <= 0.0 {
        return vec![];
    }

    let threshold_price = position.price * (1.0 - bot_params.close_trailing_threshold_pct);
    let retracement_price =
        trailing_price_bundle.min_since_open * (1.0 + bot_params.close_trailing_retracement_pct);

    if trailing_price_bundle.min_since_open < threshold_price
        && state_params.order_book.best_ask() > retracement_price
    {
        // Trailing stop triggered
        let close_qty = position.size.abs() * bot_params.close_trailing_qty_pct;
        let close_price = state_params.order_book.best_ask();

        let min_qty = super::entries::calc_min_entry_qty(close_price, exchange_params);

        if close_qty >= min_qty {
            return vec![GridOrder {
                qty: close_qty,
                price: close_price,
                order_type: OrderType::CloseTrailingShort,
            }];
        }
    }

    vec![]
}

pub fn calc_closes_short(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> Vec<GridOrder> {
    if bot_params.close_trailing_threshold_pct > 0.0
        && bot_params.close_trailing_retracement_pct > 0.0
    {
        let trailing_closes = calc_trailing_close_short(
            exchange_params,
            state_params,
            bot_params,
            position,
            trailing_price_bundle,
        );
        if !trailing_closes.is_empty() {
            return trailing_closes;
        }
    }

    let closes = if bot_params.backwards_tp {
        calc_close_grid_backwards_short(
            state_params.balance,
            position.size,
            position.price,
            state_params.order_book.best_bid(),
            state_params.ema_bands.lower,
            0.0,
            0.0,
            exchange_params.inverse,
            exchange_params.qty_step,
            exchange_params.price_step,
            exchange_params.min_qty,
            exchange_params.min_cost,
            exchange_params.c_mult,
            bot_params.total_wallet_exposure_limit,
            bot_params.close_grid_min_markup,
            bot_params.close_grid_markup_range,
            bot_params.n_close_orders,
            bot_params.unstuck_threshold,
            bot_params.unstuck_ema_dist,
            0.0, // unstuck_delay_minutes
            0.0, // unstuck_qty_pct
        )
    } else {
        calc_close_grid_frontwards_short(
            state_params.balance,
            position.size,
            position.price,
            state_params.order_book.best_bid(),
            state_params.ema_bands.lower,
            0.0,
            0.0,
            exchange_params.inverse,
            exchange_params.qty_step,
            exchange_params.price_step,
            exchange_params.min_qty,
            exchange_params.min_cost,
            exchange_params.c_mult,
            bot_params.total_wallet_exposure_limit,
            bot_params.close_grid_min_markup,
            bot_params.close_grid_markup_range,
            bot_params.n_close_orders,
            bot_params.unstuck_threshold,
            bot_params.unstuck_ema_dist,
            0.0, // unstuck_delay_minutes
            0.0, // unstuck_qty_pct
        )
    };
    closes
        .iter()
        .filter_map(
            |(qty, price, order_type_str)| match OrderType::from_str(order_type_str) {
                Some(order_type) => Some(GridOrder {
                    qty: *qty,
                    price: *price,
                    order_type,
                }),
                None => {
                    warn!("Unknown order type string: {}", order_type_str);
                    None
                }
            },
        )
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BotSideConfig, EMABands, ExchangeParams, OrderBook, Position, StateParams};

    fn setup_test_params() -> (ExchangeParams, StateParams, BotSideConfig, Position) {
        let exchange_params = ExchangeParams {
            qty_step: 0.001,
            price_step: 0.01,
            min_qty: 0.001,
            min_cost: 1.0,
            c_mult: 1.0,
            inverse: false,
        };

        let state_params = StateParams {
            balance: 1000.0,
            order_book: OrderBook {
                bids: vec![[99.0, 1.0]],
                asks: vec![[101.0, 1.0]],
            },
            ema_bands: EMABands {
                upper: 105.0,
                lower: 95.0,
            },
        };

        let bot_params = BotSideConfig {
            backwards_tp: false,
            total_wallet_exposure_limit: 10.0,
            close_grid_min_markup: 0.01,
            close_grid_markup_range: 0.02,
            n_close_orders: 5.0,
            unstuck_threshold: 0.1,
            unstuck_ema_dist: 0.01,
            ..Default::default()
        };

        let position = Position {
            size: 1.0,
            price: 100.0,
        };

        (exchange_params, state_params, bot_params, position)
    }

    #[test]
    fn test_calc_closes_long_frontwards() {
        let (exchange_params, state_params, mut bot_params, position) = setup_test_params();
        bot_params.backwards_tp = false;

        let trailing_bundle = TrailingPriceBundle::default();

        let closes = calc_closes_long(
            &exchange_params,
            &state_params,
            &bot_params,
            &position,
            &trailing_bundle,
        );

        assert!(!closes.is_empty());
        assert_eq!(closes.len(), 5);

        let total_qty: f64 = closes.iter().map(|o| o.qty.abs()).sum();
        assert!((total_qty - position.size).abs() < exchange_params.qty_step);

        for i in 0..closes.len() - 1 {
            assert!(closes[i].price < closes[i + 1].price);
        }
    }

    #[test]
    fn test_calc_closes_long_backwards() {
        let (exchange_params, state_params, mut bot_params, position) = setup_test_params();
        bot_params.backwards_tp = true;

        let trailing_bundle = TrailingPriceBundle::default();

        let closes = calc_closes_long(
            &exchange_params,
            &state_params,
            &bot_params,
            &position,
            &trailing_bundle,
        );

        assert!(!closes.is_empty());
        assert_eq!(closes.len(), 1);

        let total_qty: f64 = closes.iter().map(|o| o.qty.abs()).sum();
        assert!((total_qty - position.size).abs() < exchange_params.qty_step);
    }
}
