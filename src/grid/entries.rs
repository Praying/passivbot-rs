use crate::types::{
    BotSideConfig, ExchangeParams, GridOrder, OrderBook, OrderType, Position, StateParams,
    TrailingPriceBundle,
};
use super::utils::{
    calc_ema_price_ask, calc_ema_price_bid, calc_new_psize_pprice, calc_wallet_exposure,
    calc_wallet_exposure_if_filled, cost_to_qty, interpolate, round_, round_dn, round_up,
};
use crate::grid::utils::{calc_pnl_long, calc_pnl_short};
use std::cmp::Ordering;

/// Iteratively finds an entry order quantity that brings the wallet exposure
/// to the target limit defined in `bot_params.total_wallet_exposure_limit`.
///
/// This function uses an iterative approach (up to 15 iterations) to find an
/// order quantity. It starts with a guess and refines it using interpolation
/// until the resulting wallet exposure is within 1% of the target limit.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side (long or short).
/// * `state_params` - Current state parameters like balance and order book.
/// * `position` - The current position details.
/// * `entry_price` - The price at which the entry order would be executed.
///
/// # Returns
///
/// The calculated quantity that best matches the target wallet exposure.
/// Returns 0.0 if the current wallet exposure is already near or above the limit.
pub fn find_entry_qty_bringing_wallet_exposure_to_target(
    exchange_params: &ExchangeParams, bot_params: &BotSideConfig, state_params: &StateParams,
    position: &Position, entry_price: f64,
) -> f64 {
    let wallet_exposure = calc_wallet_exposure(
        exchange_params.c_mult,
        state_params.balance,
        position.size,
        position.price,
        exchange_params.inverse,
    );

    if wallet_exposure >= bot_params.total_wallet_exposure_limit * 0.99 {
        return 0.0;
    }

    let mut guesses = vec![];
    let mut vals = vec![];
    let mut evals = vec![];

    guesses.push(round_(
        position.size.abs() * bot_params.total_wallet_exposure_limit
            / f64::max(0.01, wallet_exposure),
        exchange_params.qty_step,
    ));
    vals.push(calc_wallet_exposure_if_filled(
        state_params.balance,
        position.size,
        position.price,
        guesses.last().unwrap().clone(),
        entry_price,
        exchange_params.inverse,
        &exchange_params,
    ));
    evals.push(
        (vals.last().unwrap().clone() - bot_params.total_wallet_exposure_limit).abs()
            / bot_params.total_wallet_exposure_limit,
    );

    guesses.push(f64::max(
        0.0,
        round_(
            f64::max(
                guesses.last().unwrap().clone() * 1.2,
                guesses.last().unwrap().clone() + exchange_params.qty_step,
            ),
            exchange_params.qty_step,
        ),
    ));
    vals.push(calc_wallet_exposure_if_filled(
        state_params.balance,
        position.size,
        position.price,
        guesses.last().unwrap().clone(),
        entry_price,
        exchange_params.inverse,
        &exchange_params,
    ));
    evals.push(
        (vals.last().unwrap().clone() - bot_params.total_wallet_exposure_limit).abs()
            / bot_params.total_wallet_exposure_limit,
    );

    for _ in 0..15 {
        if guesses.last().unwrap() == &guesses[guesses.len() - 2] {
            guesses.push(
                (guesses[guesses.len() - 2] * 1.1)
                    .max(guesses[guesses.len() - 2] + exchange_params.qty_step)
                    .abs(),
            );
            vals.push(calc_wallet_exposure_if_filled(
                state_params.balance,
                position.size,
                position.price,
                guesses.last().unwrap().clone(),
                entry_price,
                exchange_params.inverse,
                &exchange_params,
            ));
        }
        guesses.push(f64::max(
            0.0,
            round_(
                interpolate(
                    bot_params.total_wallet_exposure_limit,
                    &vals[vals.len() - 2..],
                    &guesses[guesses.len() - 2..],
                ),
                exchange_params.qty_step,
            ),
        ));
        vals.push(calc_wallet_exposure_if_filled(
            state_params.balance,
            position.size,
            position.price,
            guesses.last().unwrap().clone(),
            entry_price,
            exchange_params.inverse,
            &exchange_params,
        ));
        evals.push(
            (vals.last().unwrap().clone() - bot_params.total_wallet_exposure_limit).abs()
                / bot_params.total_wallet_exposure_limit,
        );

        if evals.last().unwrap() < &0.01 {
            break;
        }
    }

    let mut evals_guesses: Vec<_> = evals.iter().zip(guesses.iter()).collect();
    evals_guesses.sort_by(|a, b| a.0.partial_cmp(b.0).unwrap_or(Ordering::Equal));

    *evals_guesses[0].1
}

/// Calculates an "auto unstuck" entry order for a long position.
///
/// This function is triggered when the position is significantly in loss. It calculates
/// an entry order at a lower price (based on EMA bands) to "average down" the
/// position price, aiming to get it "unstuck". The quantity is determined by
/// `find_entry_qty_bringing_wallet_exposure_to_target`.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side.
/// * `state_params` - Current state parameters.
/// * `position` - The current long position details.
///
/// # Returns
///
/// A `GridOrder` struct representing the calculated auto-unstuck entry order.
pub fn calc_auto_unstuck_entry_long(
    exchange_params: &ExchangeParams, bot_params: &BotSideConfig, state_params: &StateParams,
    position: &Position,
) -> GridOrder {
    let auto_unstuck_entry_price = f64::min(
        state_params.order_book.best_bid(),
        round_dn(
            state_params.ema_bands.lower * (1.0 - bot_params.unstuck_ema_dist),
            exchange_params.price_step,
        ),
    );

    let auto_unstuck_qty = find_entry_qty_bringing_wallet_exposure_to_target(
        exchange_params,
        bot_params,
        state_params,
        position,
        auto_unstuck_entry_price,
    );

    let min_entry_qty = calc_min_entry_qty(auto_unstuck_entry_price, &exchange_params);

    GridOrder {
        qty: f64::max(auto_unstuck_qty, min_entry_qty),
        price: auto_unstuck_entry_price,
        order_type: OrderType::EntryUnstuckLong,
    }
}

/// Calculates an "auto unstuck" entry order for a short position.
///
/// This function is the counterpart to `calc_auto_unstuck_entry_long`. It's triggered
/// when a short position is significantly in loss. It calculates an entry order at a
/// higher price to "average up" the position price.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side.
/// * `state_params` - Current state parameters.
/// * `position` - The current short position details.
///
/// # Returns
///
/// A `GridOrder` struct representing the calculated auto-unstuck entry order.
pub fn calc_auto_unstuck_entry_short(
    exchange_params: &ExchangeParams, bot_params: &BotSideConfig, state_params: &StateParams,
    position: &Position,
) -> GridOrder {
    let auto_unstuck_entry_price = f64::max(
        state_params.order_book.best_ask(),
        round_up(
            state_params.ema_bands.upper * (1.0 + bot_params.unstuck_ema_dist),
            exchange_params.price_step,
        ),
    );

    let auto_unstuck_qty = find_entry_qty_bringing_wallet_exposure_to_target(
        exchange_params,
        bot_params,
        state_params,
        position,
        auto_unstuck_entry_price,
    );

    let min_entry_qty = calc_min_entry_qty(auto_unstuck_entry_price, &exchange_params);

    GridOrder {
        qty: -f64::max(auto_unstuck_qty, min_entry_qty),
        price: auto_unstuck_entry_price,
        order_type: OrderType::EntryUnstuckShort,
    }
}

/// Calculates the quantity for the very first entry order.
///
/// This quantity is based on a percentage of the total wallet exposure limit,
/// ensuring the initial trade is a fraction of the total intended risk.
/// It also ensures the quantity is above the exchange's minimum order size.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side.
/// * `balance` - Current account balance.
/// * `entry_price` - The price for the initial entry.
///
/// # Returns
///
/// The calculated quantity for the initial entry order.
pub fn calc_initial_entry_qty(
    exchange_params: &ExchangeParams, bot_params: &BotSideConfig, balance: f64, entry_price: f64,
) -> f64 {
    f64::max(
        calc_min_entry_qty(entry_price, &exchange_params),
        round_(
            cost_to_qty(
                balance * bot_params.total_wallet_exposure_limit * bot_params.entry_initial_qty_pct,
                entry_price,
                exchange_params.inverse,
                exchange_params.c_mult,
            ),
            exchange_params.qty_step,
        ),
    )
}

/// Determines the minimum allowed entry quantity.
///
/// This is the greater of the exchange's absolute minimum quantity (`min_qty`)
/// and the quantity required to meet the minimum cost (`min_cost`) at the given
/// `entry_price`. This is a fundamental constraint used in many other calculations.
///
/// # Arguments
///
/// * `entry_price` - The price at which the order would be placed.
/// * `exchange_params` - General parameters for the exchange, including `min_qty` and `min_cost`.
///
/// # Returns
///
/// The minimum valid order quantity.
pub fn calc_min_entry_qty(entry_price: f64, exchange_params: &ExchangeParams) -> f64 {
    f64::max(
        exchange_params.min_qty,
        round_up(
            cost_to_qty(
                exchange_params.min_cost,
                entry_price,
                exchange_params.inverse,
                exchange_params.c_mult,
            ),
            exchange_params.qty_step,
        ),
    )
}

/// Calculates a reentry quantity, cropping it if it would exceed the wallet exposure limit.
///
/// If a proposed `entry_qty` would cause the wallet exposure to surpass the configured limit,
/// this function calculates a new, smaller ("cropped") quantity that brings the exposure
/// exactly to the limit. Otherwise, it returns the original quantity.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The current position details.
/// * `wallet_exposure` - The current wallet exposure before this trade.
/// * `balance` - Current account balance.
/// * `entry_qty` - The proposed reentry quantity.
/// * `entry_price` - The price of the proposed reentry.
///
/// # Returns
///
/// A tuple containing:
/// * `(f64)`: The projected wallet exposure if the order is filled.
/// * `(f64)`: The (potentially cropped) reentry quantity.
pub fn calc_cropped_reentry_qty(
    exchange_params: &ExchangeParams, bot_params: &BotSideConfig, position: &Position,
    wallet_exposure: f64, balance: f64, entry_qty: f64, entry_price: f64,
) -> (f64, f64) {
    let position_size_abs = position.size.abs();
    let entry_qty_abs = entry_qty.abs();
    let wallet_exposure_if_filled = calc_wallet_exposure_if_filled(
        balance,
        position_size_abs,
        position.price,
        entry_qty_abs,
        entry_price,
        exchange_params.inverse,
        &exchange_params,
    );
    let min_entry_qty = calc_min_entry_qty(entry_price, &exchange_params);
    if wallet_exposure_if_filled > bot_params.total_wallet_exposure_limit * 1.01 {
        // reentry too big. Crop current reentry qty.
        let entry_qty_abs = interpolate(
            bot_params.total_wallet_exposure_limit,
            &[wallet_exposure, wallet_exposure_if_filled],
            &[position_size_abs, position_size_abs + entry_qty_abs],
        ) - position_size_abs;
        (
            wallet_exposure_if_filled,
            f64::max(
                round_(entry_qty_abs, exchange_params.qty_step),
                min_entry_qty,
            ),
        )
    } else {
        (
            wallet_exposure_if_filled,
            f64::max(entry_qty_abs, min_entry_qty),
        )
    }
}

/// Calculates the quantity for a standard reentry order.
///
/// The reentry quantity is the larger of two values:
/// 1. A multiple of the current position size (`entry_grid_double_down_factor`).
/// 2. A quantity based on the initial entry quantity percentage (`entry_initial_qty_pct`).
///
/// This ensures that reentry orders scale with the position size while also having a
/// baseline quantity related to the total risk allocation. The result is always at
/// least the minimum required quantity.
///
/// # Arguments
///
/// * `entry_price` - The price of the proposed reentry.
/// * `balance` - Current account balance.
/// * `position_size` - The absolute size of the current position.
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side.
///
/// # Returns
///
/// The calculated quantity for the reentry order.
pub fn calc_reentry_qty(
    entry_price: f64, balance: f64, position_size: f64, exchange_params: &ExchangeParams,
    bot_params: &BotSideConfig,
) -> f64 {
    f64::max(
        calc_min_entry_qty(entry_price, &exchange_params),
        round_(
            f64::max(
                position_size.abs() * bot_params.entry_grid_double_down_factor,
                cost_to_qty(
                    balance,
                    entry_price,
                    exchange_params.inverse,
                    exchange_params.c_mult,
                ) * bot_params.total_wallet_exposure_limit
                    * bot_params.entry_initial_qty_pct,
            ),
            exchange_params.qty_step,
        ),
    )
}

/// Calculates the next grid reentry price for a long position (bid side).
///
/// The price is determined by stepping down from the current position price. The step size
/// is based on `entry_grid_spacing_pct` and is weighted by how much of the wallet
/// exposure limit has been used (`entry_grid_spacing_weight`). This creates a dynamic
/// grid that spreads out as more capital is deployed. The final price is capped by the
/// current best bid in the order book.
///
/// # Arguments
///
/// * `position_price` - The average price of the current position.
/// * `wallet_exposure` - The current wallet exposure.
/// * `order_book_bid` - The best bid price from the order book.
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side.
///
/// # Returns
///
/// The calculated reentry price for the bid side.
fn calc_reentry_price_bid(
    position_price: f64, wallet_exposure: f64, order_book_bid: f64,
    exchange_params: &ExchangeParams, bot_params: &BotSideConfig,
) -> f64 {
    let multiplier = (wallet_exposure / bot_params.total_wallet_exposure_limit)
        * bot_params.entry_grid_spacing_weight;
    let reentry_price = f64::min(
        round_dn(
            position_price * (1.0 - bot_params.entry_grid_spacing_pct * (1.0 + multiplier)),
            exchange_params.price_step,
        ),
        order_book_bid,
    );
    if reentry_price <= exchange_params.price_step {
        0.0
    } else {
        reentry_price
    }
}

/// Calculates the next grid reentry price for a short position (ask side).
///
/// This is the counterpart to `calc_reentry_price_bid`. The price is determined by
/// stepping up from the current position price, with a dynamic step size weighted by
/// wallet exposure. The final price is capped by the current best ask in the order book.
///
/// # Arguments
///
/// * `position_price` - The average price of the current position.
/// * `wallet_exposure` - The current wallet exposure.
/// * `order_book_ask` - The best ask price from the order book.
/// * `exchange_params` - General parameters for the exchange.
/// * `bot_params` - Configuration for the bot's side.
///
/// # Returns
///
/// The calculated reentry price for the ask side.
fn calc_reentry_price_ask(
    position_price: f64, wallet_exposure: f64, order_book_ask: f64,
    exchange_params: &ExchangeParams, bot_params: &BotSideConfig,
) -> f64 {
    let multiplier = (wallet_exposure / bot_params.total_wallet_exposure_limit)
        * bot_params.entry_grid_spacing_weight;
    let reentry_price = f64::max(
        round_up(
            position_price * (1.0 + bot_params.entry_grid_spacing_pct * (1.0 + multiplier)),
            exchange_params.price_step,
        ),
        order_book_ask,
    );
    if reentry_price <= exchange_params.price_step {
        0.0
    } else {
        reentry_price
    }
}

/// Calculates the next grid entry order for a long position.
///
/// This is a core logic function that determines the price and quantity for the next
/// long grid entry. It handles several cases:
/// - No position: Places an initial entry order.
/// - Partially filled initial entry: Places an order to complete the initial quantity.
/// - Normal reentry: Places a standard grid reentry order.
/// - Cropped reentry: If the standard reentry is too large, it's cropped.
/// - Inflated reentry: If the *next* potential reentry would be too small, the *current*
///   reentry is inflated to maintain the doubling-down strategy.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The current position details.
///
/// # Returns
///
/// A `GridOrder` struct representing the calculated grid entry. Returns a default
/// `GridOrder` (qty=0) if no entry should be placed.
pub fn calc_grid_entry_long(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position,
) -> GridOrder {
    if bot_params.total_wallet_exposure_limit == 0.0 || state_params.balance <= 0.0 {
        return GridOrder::default();
    }
    let initial_entry_price = calc_ema_price_bid(
        exchange_params.price_step,
        state_params.order_book.best_bid(),
        state_params.ema_bands.lower,
        bot_params.entry_initial_ema_dist,
    );
    if initial_entry_price <= exchange_params.price_step {
        return GridOrder::default();
    }
    let initial_entry_qty = calc_initial_entry_qty(
        exchange_params,
        bot_params,
        state_params.balance,
        initial_entry_price,
    );
    if position.size == 0.0 {
        return GridOrder {
            qty: initial_entry_qty,
            price: initial_entry_price,
            order_type: OrderType::EntryInitialNormalLong,
        };
    } else if position.size < initial_entry_qty * 0.8 {
        return GridOrder {
            qty: f64::max(
                calc_min_entry_qty(initial_entry_price, &exchange_params),
                round_dn(initial_entry_qty - position.size, exchange_params.qty_step),
            ),
            price: initial_entry_price,
            order_type: OrderType::EntryInitialPartialLong,
        };
    }
    let wallet_exposure = calc_wallet_exposure(
        exchange_params.c_mult,
        state_params.balance,
        position.size,
        position.price,
        exchange_params.inverse,
    );
    if wallet_exposure >= bot_params.total_wallet_exposure_limit * 0.999 {
        return GridOrder::default();
    }

    // normal re-entry
    let reentry_price = calc_reentry_price_bid(
        position.price,
        wallet_exposure,
        state_params.order_book.best_bid(),
        exchange_params,
        bot_params,
    );
    if reentry_price <= 0.0 {
        return GridOrder::default();
    }
    let reentry_qty = f64::max(
        calc_reentry_qty(
            reentry_price,
            state_params.balance,
            position.size,
            exchange_params,
            bot_params,
        ),
        initial_entry_qty,
    );
    let (wallet_exposure_if_filled, reentry_qty_cropped) = calc_cropped_reentry_qty(
        exchange_params,
        bot_params,
        position,
        wallet_exposure,
        state_params.balance,
        reentry_qty,
        reentry_price,
    );
    if reentry_qty_cropped < reentry_qty {
        return GridOrder {
            qty: reentry_qty_cropped,
            price: reentry_price,
            order_type: OrderType::EntryGridCroppedLong,
        };
    }
    // preview next order to check if reentry qty is to be inflated
    let (psize_if_filled, pprice_if_filled) = calc_new_psize_pprice(
        position.size,
        position.price,
        reentry_qty,
        reentry_price,
        exchange_params.qty_step,
    );
    let next_reentry_price = calc_reentry_price_bid(
        pprice_if_filled,
        wallet_exposure_if_filled,
        state_params.order_book.best_bid(),
        exchange_params,
        bot_params,
    );
    let next_reentry_qty = f64::max(
        calc_reentry_qty(
            next_reentry_price,
            state_params.balance,
            psize_if_filled,
            exchange_params,
            bot_params,
        ),
        initial_entry_qty,
    );
    let (_next_wallet_exposure_if_filled, next_reentry_qty_cropped) = calc_cropped_reentry_qty(
        exchange_params,
        bot_params,
        &Position {
            size: psize_if_filled,
            price: pprice_if_filled,
        },
        wallet_exposure_if_filled,
        state_params.balance,
        next_reentry_qty,
        next_reentry_price,
    );
    let effective_double_down_factor = next_reentry_qty_cropped / psize_if_filled;
    if effective_double_down_factor < bot_params.entry_grid_double_down_factor * 0.25 {
        // next reentry too small. Inflate current reentry.
        let new_entry_qty = interpolate(
            bot_params.total_wallet_exposure_limit,
            &[wallet_exposure, wallet_exposure_if_filled],
            &[position.size, position.size + reentry_qty],
        ) - position.size;
        GridOrder {
            qty: round_(new_entry_qty, exchange_params.qty_step),
            price: reentry_price,
            order_type: OrderType::EntryGridInflatedLong,
        }
    } else {
        GridOrder {
            qty: reentry_qty,
            price: reentry_price,
            order_type: OrderType::EntryGridNormalLong,
        }
    }
}

/// Determines the next entry order for a long position, choosing between grid and trailing.
///
/// This function acts as a router based on `bot_params.entry_trailing_grid_ratio`.
/// - If ratio is 0, it always uses `calc_grid_entry_long`.
/// - If ratio is >= 1.0 or <= -1.0, it always uses `calc_trailing_entry_long`.
/// - If ratio is between 0 and 1 (e.g., 0.4), it uses trailing orders until the wallet
///   exposure ratio reaches the ratio value, then switches to grid orders.
/// - If ratio is between -1 and 0 (e.g., -0.6), it uses grid orders first, then trailing.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The current position details.
/// * `trailing_price_bundle` - Price tracking data for trailing logic.
///
/// # Returns
///
/// The `GridOrder` determined by either the grid or trailing logic.
pub fn calc_next_entry_long(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> GridOrder {
    // determines whether trailing or grid order, returns GridOrder
    if bot_params.total_wallet_exposure_limit == 0.0 || state_params.balance <= 0.0 {
        // no orders
        return GridOrder::default();
    }
    if bot_params.entry_trailing_grid_ratio >= 1.0 || bot_params.entry_trailing_grid_ratio <= -1.0 {
        // return trailing only
        return calc_trailing_entry_long(
            &exchange_params,
            &state_params,
            &bot_params,
            &position,
            &trailing_price_bundle,
        );
    } else if bot_params.entry_trailing_grid_ratio == 0.0 {
        // return grid only
        return calc_grid_entry_long(&exchange_params, &state_params, &bot_params, &position);
    }
    let wallet_exposure = calc_wallet_exposure(
        exchange_params.c_mult,
        state_params.balance,
        position.size,
        position.price,
        exchange_params.inverse,
    );
    let wallet_exposure_ratio = wallet_exposure / bot_params.total_wallet_exposure_limit;
    if bot_params.entry_trailing_grid_ratio > 0.0 {
        // trailing first
        if wallet_exposure_ratio < bot_params.entry_trailing_grid_ratio {
            // return trailing order, but crop to max bot_params.total_wallet_exposure_limit * bot_params.entry_trailing_grid_ratio + 1%
            if wallet_exposure == 0.0 {
                calc_trailing_entry_long(
                    &exchange_params,
                    &state_params,
                    &bot_params,
                    &position,
                    &trailing_price_bundle,
                )
            } else {
                let mut bot_params_modified = bot_params.clone();
                bot_params_modified.total_wallet_exposure_limit = bot_params
                    .total_wallet_exposure_limit
                    * bot_params.entry_trailing_grid_ratio
                    * 1.01;
                calc_trailing_entry_long(
                    &exchange_params,
                    &state_params,
                    &bot_params_modified,
                    &position,
                    &trailing_price_bundle,
                )
            }
        } else {
            // return grid order
            calc_grid_entry_long(&exchange_params, &state_params, &bot_params, &position)
        }
    } else {
        // grid first
        if wallet_exposure_ratio < 1.0 + bot_params.entry_trailing_grid_ratio {
            // return grid order, but crop to max bot_params.total_wallet_exposure_limit * (1.0 + bot_params.entry_trailing_grid_ratio) + 1%
            if wallet_exposure == 0.0 {
                calc_grid_entry_long(&exchange_params, &state_params, &bot_params, &position)
            } else {
                let mut bot_params_modified = bot_params.clone();
                if wallet_exposure != 0.0 {
                    bot_params_modified.total_wallet_exposure_limit = bot_params
                        .total_wallet_exposure_limit
                        * (1.0 + bot_params.entry_trailing_grid_ratio)
                        * 1.01;
                }
                calc_grid_entry_long(
                    &exchange_params,
                    &state_params,
                    &bot_params_modified,
                    &position,
                )
            }
        } else {
            calc_trailing_entry_long(
                &exchange_params,
                &state_params,
                &bot_params,
                &position,
                &trailing_price_bundle,
            )
        }
    }
}

/// Calculates the next trailing entry order for a long position.
///
/// This function implements the trailing entry logic. It triggers an entry based on
/// price retracement from recent lows (`min_since_open`).
/// - It first checks for initial or partial entries, similar to the grid logic.
/// - It then evaluates if the trailing conditions (threshold and retracement) are met.
/// - If triggered, it calculates the reentry quantity and crops it if necessary.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The current position details.
/// * `trailing_price_bundle` - Price tracking data for trailing logic.
///
/// # Returns
///
/// A `GridOrder` struct for the trailing entry. Returns a default `GridOrder` (qty=0)
/// if trailing conditions are not met.
pub fn calc_trailing_entry_long(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> GridOrder {
    let initial_entry_price = calc_ema_price_bid(
        exchange_params.price_step,
        state_params.order_book.best_bid(),
        state_params.ema_bands.lower,
        bot_params.entry_initial_ema_dist,
    );
    if initial_entry_price <= exchange_params.price_step {
        return GridOrder::default();
    }
    let initial_entry_qty = calc_initial_entry_qty(
        exchange_params,
        bot_params,
        state_params.balance,
        initial_entry_price,
    );
    if position.size == 0.0 {
        // normal initial entry
        return GridOrder {
            qty: initial_entry_qty,
            price: initial_entry_price,
            order_type: OrderType::EntryInitialNormalLong,
        };
    } else if position.size < initial_entry_qty * 0.8 {
        return GridOrder {
            qty: f64::max(
                calc_min_entry_qty(initial_entry_price, &exchange_params),
                round_dn(initial_entry_qty - position.size, exchange_params.qty_step),
            ),
            price: initial_entry_price,
            order_type: OrderType::EntryInitialPartialLong,
        };
    }
    let wallet_exposure = calc_wallet_exposure(
        exchange_params.c_mult,
        state_params.balance,
        position.size,
        position.price,
        exchange_params.inverse,
    );
    if wallet_exposure > bot_params.total_wallet_exposure_limit * 0.999 {
        return GridOrder::default();
    }
    let mut entry_triggered = false;
    let mut reentry_price = 0.0;
    if bot_params.entry_trailing_threshold_pct <= 0.0 {
        // means trailing entry immediately from pos change
        if bot_params.entry_trailing_retracement_pct > 0.0
            && trailing_price_bundle.max_since_min
                > trailing_price_bundle.min_since_open
                    * (1.0 + bot_params.entry_trailing_retracement_pct)
        {
            entry_triggered = true;
            reentry_price = state_params.order_book.best_bid();
        }
    } else {
        // means trailing entry will activate only after a threshold
        if bot_params.entry_trailing_retracement_pct <= 0.0 {
            // close at threshold
            entry_triggered = true;
            reentry_price = f64::min(
                state_params.order_book.best_bid(),
                round_dn(
                    position.price * (1.0 - bot_params.entry_trailing_threshold_pct),
                    exchange_params.price_step,
                ),
            );
        } else {
            // enter if both conditions are met
            if trailing_price_bundle.min_since_open
                < position.price * (1.0 - bot_params.entry_trailing_threshold_pct)
                && trailing_price_bundle.max_since_min
                    > trailing_price_bundle.min_since_open
                        * (1.0 + bot_params.entry_trailing_retracement_pct)
            {
                entry_triggered = true;
                reentry_price = f64::min(
                    state_params.order_book.best_bid(),
                    round_dn(
                        position.price
                            * (1.0 - bot_params.entry_trailing_threshold_pct
                                + bot_params.entry_trailing_retracement_pct),
                        exchange_params.price_step,
                    ),
                );
            }
        }
    }
    if !entry_triggered {
        return GridOrder {
            qty: 0.0,
            price: 0.0,
            order_type: OrderType::EntryTrailingNormalLong,
        };
    }
    let reentry_qty = f64::max(
        calc_reentry_qty(
            reentry_price,
            state_params.balance,
            position.size,
            &exchange_params,
            &bot_params,
        ),
        initial_entry_qty,
    );
    let (_wallet_exposure_if_filled, reentry_qty_cropped) = calc_cropped_reentry_qty(
        exchange_params,
        bot_params,
        position,
        wallet_exposure,
        state_params.balance,
        reentry_qty,
        reentry_price,
    );
    if reentry_qty_cropped < reentry_qty {
        GridOrder {
            qty: reentry_qty_cropped,
            price: reentry_price,
            order_type: OrderType::EntryTrailingCroppedLong,
        }
    } else {
        GridOrder {
            qty: reentry_qty,
            price: reentry_price,
            order_type: OrderType::EntryTrailingNormalLong,
        }
    }
}

/// Calculates the next grid entry order for a short position.
///
/// This is the short-side counterpart to `calc_grid_entry_long`. It determines the
/// price and quantity for the next short grid entry, handling initial, partial, normal,
/// cropped, and inflated reentry scenarios.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The current position details.
///
/// # Returns
///
/// A `GridOrder` struct representing the calculated grid entry. Returns a default
/// `GridOrder` (qty=0) if no entry should be placed.
pub fn calc_grid_entry_short(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position,
) -> GridOrder {
    if bot_params.total_wallet_exposure_limit == 0.0 || state_params.balance <= 0.0 {
        return GridOrder::default();
    }
    let initial_entry_price = calc_ema_price_ask(
        exchange_params.price_step,
        state_params.order_book.best_ask(),
        state_params.ema_bands.upper,
        bot_params.entry_initial_ema_dist,
    );
    if initial_entry_price <= exchange_params.price_step {
        return GridOrder::default();
    }
    let initial_entry_qty = calc_initial_entry_qty(
        exchange_params,
        bot_params,
        state_params.balance,
        initial_entry_price,
    );
    let position_size_abs = position.size.abs();
    if position_size_abs == 0.0 {
        return GridOrder {
            qty: -initial_entry_qty,
            price: initial_entry_price,
            order_type: OrderType::EntryInitialNormalShort,
        };
    } else if position_size_abs < initial_entry_qty * 0.8 {
        return GridOrder {
            qty: -f64::max(
                calc_min_entry_qty(initial_entry_price, &exchange_params),
                round_dn(
                    initial_entry_qty - position_size_abs,
                    exchange_params.qty_step,
                ),
            ),
            price: initial_entry_price,
            order_type: OrderType::EntryInitialPartialShort,
        };
    }
    let wallet_exposure = calc_wallet_exposure(
        exchange_params.c_mult,
        state_params.balance,
        position_size_abs,
        position.price,
        exchange_params.inverse,
    );
    if wallet_exposure >= bot_params.total_wallet_exposure_limit * 0.999 {
        return GridOrder::default();
    }

    // normal re-entry
    let reentry_price = calc_reentry_price_ask(
        position.price,
        wallet_exposure,
        state_params.order_book.best_ask(),
        exchange_params,
        bot_params,
    );
    if reentry_price <= 0.0 {
        return GridOrder::default();
    }
    let reentry_qty = f64::max(
        calc_reentry_qty(
            reentry_price,
            state_params.balance,
            position_size_abs,
            exchange_params,
            bot_params,
        ),
        initial_entry_qty,
    );
    let (wallet_exposure_if_filled, reentry_qty_cropped) = calc_cropped_reentry_qty(
        exchange_params,
        bot_params,
        position,
        wallet_exposure,
        state_params.balance,
        reentry_qty,
        reentry_price,
    );
    if reentry_qty_cropped < reentry_qty {
        return GridOrder {
            qty: -reentry_qty_cropped,
            price: reentry_price,
            order_type: OrderType::EntryGridCroppedShort,
        };
    }
    // preview next order to check if reentry qty is to be inflated
    let (psize_if_filled, pprice_if_filled) = calc_new_psize_pprice(
        position_size_abs,
        position.price,
        reentry_qty,
        reentry_price,
        exchange_params.qty_step,
    );
    let next_reentry_price = calc_reentry_price_ask(
        pprice_if_filled,
        wallet_exposure_if_filled,
        state_params.order_book.best_ask(),
        exchange_params,
        bot_params,
    );
    let next_reentry_qty = f64::max(
        calc_reentry_qty(
            next_reentry_price,
            state_params.balance,
            psize_if_filled,
            exchange_params,
            bot_params,
        ),
        initial_entry_qty,
    );
    let (_next_wallet_exposure_if_filled, next_reentry_qty_cropped) = calc_cropped_reentry_qty(
        exchange_params,
        bot_params,
        &Position {
            size: psize_if_filled,
            price: pprice_if_filled,
        },
        wallet_exposure_if_filled,
        state_params.balance,
        next_reentry_qty,
        next_reentry_price,
    );
    let effective_double_down_factor = next_reentry_qty_cropped / psize_if_filled;
    if effective_double_down_factor < bot_params.entry_grid_double_down_factor * 0.25 {
        // next reentry too small. Inflate current reentry.
        let new_entry_qty = interpolate(
            bot_params.total_wallet_exposure_limit,
            &[wallet_exposure, wallet_exposure_if_filled],
            &[position_size_abs, position_size_abs + reentry_qty],
        ) - position_size_abs;
        GridOrder {
            qty: -round_(new_entry_qty, exchange_params.qty_step),
            price: reentry_price,
            order_type: OrderType::EntryGridInflatedShort,
        }
    } else {
        GridOrder {
            qty: -reentry_qty,
            price: reentry_price,
            order_type: OrderType::EntryGridNormalShort,
        }
    }
}

/// Calculates the next trailing entry order for a short position.
///
/// This function implements the short-side trailing entry logic, counterpart to
/// `calc_trailing_entry_long`. It triggers an entry based on price retracement
/// from recent highs (`max_since_open`).
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The current position details.
/// * `trailing_price_bundle` - Price tracking data for trailing logic.
///
/// # Returns
///
/// A `GridOrder` struct for the trailing entry. Returns a default `GridOrder` (qty=0)
/// if trailing conditions are not met.
pub fn calc_trailing_entry_short(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> GridOrder {
    let initial_entry_price = calc_ema_price_ask(
        exchange_params.price_step,
        state_params.order_book.best_ask(),
        state_params.ema_bands.upper,
        bot_params.entry_initial_ema_dist,
    );
    if initial_entry_price <= exchange_params.price_step {
        return GridOrder::default();
    }
    let initial_entry_qty = calc_initial_entry_qty(
        exchange_params,
        bot_params,
        state_params.balance,
        initial_entry_price,
    );
    let position_size_abs = position.size.abs();
    if position_size_abs == 0.0 {
        // normal initial entry
        return GridOrder {
            qty: -initial_entry_qty,
            price: initial_entry_price,
            order_type: OrderType::EntryInitialNormalShort,
        };
    } else if position_size_abs < initial_entry_qty * 0.8 {
        return GridOrder {
            qty: -f64::max(
                calc_min_entry_qty(initial_entry_price, &exchange_params),
                round_dn(
                    initial_entry_qty - position_size_abs,
                    exchange_params.qty_step,
                ),
            ),
            price: initial_entry_price,
            order_type: OrderType::EntryInitialPartialShort,
        };
    }
    let wallet_exposure = calc_wallet_exposure(
        exchange_params.c_mult,
        state_params.balance,
        position_size_abs,
        position.price,
        exchange_params.inverse,
    );
    if wallet_exposure > bot_params.total_wallet_exposure_limit * 0.999 {
        return GridOrder::default();
    }
    let mut entry_triggered = false;
    let mut reentry_price = 0.0;
    if bot_params.entry_trailing_threshold_pct <= 0.0 {
        // means trailing entry immediately from pos change
        if bot_params.entry_trailing_retracement_pct > 0.0
            && trailing_price_bundle.min_since_max
                < trailing_price_bundle.max_since_open
                    * (1.0 - bot_params.entry_trailing_retracement_pct)
        {
            entry_triggered = true;
            reentry_price = state_params.order_book.best_ask();
        }
    } else {
        // means trailing entry will activate only after a threshold
        if bot_params.entry_trailing_retracement_pct <= 0.0 {
            // enter at threshold
            entry_triggered = true;
            reentry_price = f64::max(
                state_params.order_book.best_ask(),
                round_up(
                    position.price * (1.0 + bot_params.entry_trailing_threshold_pct),
                    exchange_params.price_step,
                ),
            );
        } else {
            // enter if both conditions are met
            if trailing_price_bundle.max_since_open
                > position.price * (1.0 + bot_params.entry_trailing_threshold_pct)
                && trailing_price_bundle.min_since_max
                    < trailing_price_bundle.max_since_open
                        * (1.0 - bot_params.entry_trailing_retracement_pct)
            {
                entry_triggered = true;
                reentry_price = f64::max(
                    state_params.order_book.best_ask(),
                    round_up(
                        position.price
                            * (1.0 + bot_params.entry_trailing_threshold_pct
                                - bot_params.entry_trailing_retracement_pct),
                        exchange_params.price_step,
                    ),
                );
            }
        }
    }
    if !entry_triggered {
        return GridOrder {
            qty: 0.0,
            price: 0.0,
            order_type: OrderType::EntryTrailingNormalShort,
        };
    }
    let reentry_qty = f64::max(
        calc_reentry_qty(
            reentry_price,
            state_params.balance,
            position_size_abs,
            &exchange_params,
            &bot_params,
        ),
        initial_entry_qty,
    );
    let (_wallet_exposure_if_filled, reentry_qty_cropped) = calc_cropped_reentry_qty(
        exchange_params,
        bot_params,
        position,
        wallet_exposure,
        state_params.balance,
        reentry_qty,
        reentry_price,
    );
    if reentry_qty_cropped < reentry_qty {
        GridOrder {
            qty: -reentry_qty_cropped,
            price: reentry_price,
            order_type: OrderType::EntryTrailingCroppedShort,
        }
    } else {
        GridOrder {
            qty: -reentry_qty,
            price: reentry_price,
            order_type: OrderType::EntryTrailingNormalShort,
        }
    }
}

/// Determines the next entry order for a short position, choosing between grid and trailing.
///
/// This function is the short-side counterpart to `calc_next_entry_long`. It acts as a
/// router based on `bot_params.entry_trailing_grid_ratio` to decide whether to call
/// `calc_grid_entry_short` or `calc_trailing_entry_short`.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The current position details.
/// * `trailing_price_bundle` - Price tracking data for trailing logic.
///
/// # Returns
///
/// The `GridOrder` determined by either the grid or trailing logic.
pub fn calc_next_entry_short(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> GridOrder {
    // determines whether trailing or grid order, returns GridOrder
    if bot_params.total_wallet_exposure_limit == 0.0 || state_params.balance <= 0.0 {
        // no orders
        return GridOrder::default();
    }
    if bot_params.entry_trailing_grid_ratio >= 1.0 || bot_params.entry_trailing_grid_ratio <= -1.0 {
        // return trailing only
        return calc_trailing_entry_short(
            &exchange_params,
            &state_params,
            &bot_params,
            &position,
            &trailing_price_bundle,
        );
    } else if bot_params.entry_trailing_grid_ratio == 0.0 {
        // return grid only
        return calc_grid_entry_short(&exchange_params, &state_params, &bot_params, &position);
    }
    let wallet_exposure = calc_wallet_exposure(
        exchange_params.c_mult,
        state_params.balance,
        position.size.abs(),
        position.price,
        exchange_params.inverse,
    );
    let wallet_exposure_ratio = wallet_exposure / bot_params.total_wallet_exposure_limit;
    if bot_params.entry_trailing_grid_ratio > 0.0 {
        // trailing first
        if wallet_exposure_ratio < bot_params.entry_trailing_grid_ratio {
            if wallet_exposure == 0.0 {
                calc_trailing_entry_short(
                    &exchange_params,
                    &state_params,
                    &bot_params,
                    &position,
                    &trailing_price_bundle,
                )
            } else {
                // return trailing order, but crop to max bot_params.total_wallet_exposure_limit * bot_params.entry_trailing_grid_ratio + 1%
                let mut bot_params_modified = bot_params.clone();
                bot_params_modified.total_wallet_exposure_limit = bot_params
                    .total_wallet_exposure_limit
                    * bot_params.entry_trailing_grid_ratio
                    * 1.01;
                calc_trailing_entry_short(
                    &exchange_params,
                    &state_params,
                    &bot_params_modified,
                    &position,
                    &trailing_price_bundle,
                )
            }
        } else {
            // return grid order
            calc_grid_entry_short(&exchange_params, &state_params, &bot_params, &position)
        }
    } else {
        // grid first
        if wallet_exposure_ratio < 1.0 + bot_params.entry_trailing_grid_ratio {
            // return grid order, but crop to max bot_params.total_wallet_exposure_limit * (1.0 + bot_params.entry_trailing_grid_ratio) + 1%
            if wallet_exposure == 0.0 {
                calc_grid_entry_short(&exchange_params, &state_params, &bot_params, &position)
            } else {
                let mut bot_params_modified = bot_params.clone();
                if wallet_exposure != 0.0 {
                    bot_params_modified.total_wallet_exposure_limit = bot_params
                        .total_wallet_exposure_limit
                        * (1.0 + bot_params.entry_trailing_grid_ratio)
                        * 1.01;
                }
                calc_grid_entry_short(
                    &exchange_params,
                    &state_params,
                    &bot_params_modified,
                    &position,
                )
            }
        } else {
            calc_trailing_entry_short(
                &exchange_params,
                &state_params,
                &bot_params,
                &position,
                &trailing_price_bundle,
            )
        }
    }
}

/// Calculates a full grid of potential future long entry orders.
///
/// This function simulates future trades to build a list of the next 500 potential
/// entry orders. It starts by checking for an "auto unstuck" order. Then, it repeatedly
/// calls `calc_next_entry_long` in a loop, updating a simulated position state after
/// each potential order, to determine the subsequent orders.
///
/// The loop stops if an order with quantity 0 is returned, or if a trailing order
/// is generated (as trailing orders are not part of a predictable grid).
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The *actual* current position details.
/// * `trailing_price_bundle` - Price tracking data for trailing logic.
///
/// # Returns
///
/// A `Vec<GridOrder>` containing the calculated grid of entry orders.
pub fn calc_entries_long(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> Vec<GridOrder> {
    let mut entries = Vec::<GridOrder>::new();

    let pos_pnl_pct = calc_pnl_long(
        position.price,
        state_params.order_book.best_bid(),
        position.size,
        exchange_params.inverse,
        exchange_params.c_mult,
    ) / state_params.balance;

    if -pos_pnl_pct > bot_params.unstuck_threshold
        && state_params.order_book.best_bid() / state_params.ema_bands.lower - 1.0
            > bot_params.unstuck_ema_dist
    {
        entries.push(calc_auto_unstuck_entry_long(
            exchange_params,
            bot_params,
            state_params,
            position,
        ));
    }

    let mut psize = position.size;
    let mut pprice = position.price;
    let mut bid = state_params.order_book.best_bid();
    for _ in 0..500 {
        let position_mod = Position {
            size: psize,
            price: pprice,
        };
        let state_params_mod = StateParams {
            balance: state_params.balance,
            ema_bands: state_params.ema_bands.clone(),
            order_book: OrderBook {
                asks: vec![],
                bids: vec![[bid, 0.0]],
            },
        };
        let entry = calc_next_entry_long(
            exchange_params,
            &state_params_mod,
            bot_params,
            &position_mod,
            &trailing_price_bundle,
        );
        if entry.qty == 0.0 {
            break;
        }
        if !entries.is_empty() {
            if entry.order_type == OrderType::EntryTrailingNormalLong
                || entry.order_type == OrderType::EntryTrailingCroppedLong
            {
                break;
            }
            if entries[entries.len() - 1].price == entry.price {
                break;
            }
        }
        (psize, pprice) = calc_new_psize_pprice(
            psize,
            pprice,
            entry.qty,
            entry.price,
            exchange_params.qty_step,
        );
        bid = bid.min(entry.price);
        entries.push(entry);
    }
    entries
}

/// Calculates a full grid of potential future short entry orders.
///
/// This is the short-side counterpart to `calc_entries_long`. It simulates future
/// short trades by repeatedly calling `calc_next_entry_short` to build a list of
/// the next 500 potential entry orders.
///
/// # Arguments
///
/// * `exchange_params` - General parameters for the exchange.
/// * `state_params` - Current state parameters.
/// * `bot_params` - Configuration for the bot's side.
/// * `position` - The *actual* current position details.
/// * `trailing_price_bundle` - Price tracking data for trailing logic.
///
/// # Returns
///
/// A `Vec<GridOrder>` containing the calculated grid of entry orders.
pub fn calc_entries_short(
    exchange_params: &ExchangeParams, state_params: &StateParams, bot_params: &BotSideConfig,
    position: &Position, trailing_price_bundle: &TrailingPriceBundle,
) -> Vec<GridOrder> {
    let mut entries = Vec::<GridOrder>::new();

    let pos_pnl_pct = calc_pnl_short(
        position.price,
        state_params.order_book.best_ask(),
        position.size,
        exchange_params.inverse,
        exchange_params.c_mult,
    ) / state_params.balance;

    if -pos_pnl_pct > bot_params.unstuck_threshold
        && state_params.ema_bands.upper / state_params.order_book.best_ask() - 1.0
            > bot_params.unstuck_ema_dist
    {
        entries.push(calc_auto_unstuck_entry_short(
            exchange_params,
            bot_params,
            state_params,
            position,
        ));
    }

    let mut psize = position.size;
    let mut pprice = position.price;
    let mut ask = state_params.order_book.best_ask();
    for _ in 0..500 {
        let position_mod = Position {
            size: psize,
            price: pprice,
        };
        let state_params_mod = StateParams {
            balance: state_params.balance,
            ema_bands: state_params.ema_bands.clone(),
            order_book: OrderBook {
                asks: vec![[ask, 0.0]],
                bids: vec![],
            },
        };
        let entry = calc_next_entry_short(
            exchange_params,
            &state_params_mod,
            bot_params,
            &position_mod,
            &trailing_price_bundle,
        );
        if entry.qty == 0.0 {
            break;
        }
        if !entries.is_empty() {
            if entry.order_type == OrderType::EntryTrailingNormalShort
                || entry.order_type == OrderType::EntryTrailingCroppedShort
            {
                break;
            }
            if entries[entries.len() - 1].price == entry.price {
                break;
            }
        }
        (psize, pprice) = calc_new_psize_pprice(
            psize,
            pprice,
            entry.qty,
            entry.price,
            exchange_params.qty_step,
        );
        ask = ask.max(entry.price);
        entries.push(entry);
    }
    entries
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        BotSideConfig, EMABands, ExchangeParams, OrderBook, Position, StateParams,
        TrailingPriceBundle,
    };

    fn setup_test_params() -> (
        ExchangeParams,
        StateParams,
        BotSideConfig,
        Position,
        TrailingPriceBundle,
    ) {
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
            total_wallet_exposure_limit: 10.0,
            entry_initial_ema_dist: 0.001,
            entry_initial_qty_pct: 0.01,
            entry_grid_spacing_pct: 0.01,
            entry_grid_spacing_weight: 1.0,
            entry_grid_double_down_factor: 2.0,
            entry_trailing_grid_ratio: 0.0,
            entry_trailing_threshold_pct: 0.005,
            entry_trailing_retracement_pct: 0.005,
            unstuck_threshold: 0.1,
            unstuck_ema_dist: 0.01,
            ..Default::default()
        };

        let position = Position {
            size: 1.0,
            price: 100.0,
        };

        let trailing_bundle = TrailingPriceBundle::default();

        (
            exchange_params,
            state_params,
            bot_params,
            position,
            trailing_bundle,
        )
    }

    #[test]
    fn test_calc_min_entry_qty() {
        let (exchange_params, _, _, _, _) = setup_test_params();
        let entry_price = 100.0;
        let min_qty = calc_min_entry_qty(entry_price, &exchange_params);
        assert_eq!(min_qty, 0.01);
    }

    #[test]
    fn test_calc_initial_entry_qty() {
        let (exchange_params, state_params, bot_params, _, _) = setup_test_params();
        let entry_price = 100.0;
        let initial_qty = calc_initial_entry_qty(
            &exchange_params,
            &bot_params,
            state_params.balance,
            entry_price,
        );
        // balance * total_wallet_exposure_limit * entry_initial_qty_pct = 1000 * 10 * 0.01 = 100
        // cost_to_qty(100, 100.0, false, 1.0) = 1.0
        // round_(1.0, 0.001) = 1.0
        // max(min_entry_qty, 1.0) = max(0.01, 1.0) = 1.0
        assert_eq!(initial_qty, 1.0);
    }

    #[test]
    fn test_calc_reentry_price_bid() {
        let (exchange_params, state_params, bot_params, position, _) = setup_test_params();
        let wallet_exposure = 5.0;
        let reentry_price = calc_reentry_price_bid(
            position.price,
            wallet_exposure,
            state_params.order_book.best_bid(),
            &exchange_params,
            &bot_params,
        );
        // multiplier = (5.0 / 10.0) * 1.0 = 0.5
        // reentry_price = min(round_dn(100.0 * (1.0 - 0.01 * (1.0 + 0.5)), 0.01), 99.0)
        // reentry_price = min(round_dn(100.0 * (1.0 - 0.015), 0.01), 99.0)
        // reentry_price = min(round_dn(98.5, 0.01), 99.0)
        // reentry_price = min(98.5, 99.0) = 98.5
        assert_eq!(reentry_price, 98.5);
    }

    #[test]
    fn test_calc_reentry_price_ask() {
        let (exchange_params, state_params, bot_params, position, _) = setup_test_params();
        let wallet_exposure = 5.0;
        let reentry_price = calc_reentry_price_ask(
            position.price,
            wallet_exposure,
            state_params.order_book.best_ask(),
            &exchange_params,
            &bot_params,
        );
        // multiplier = (5.0 / 10.0) * 1.0 = 0.5
        // reentry_price = max(round_up(100.0 * (1.0 + 0.01 * (1.0 + 0.5)), 0.01), 101.0)
        // reentry_price = max(round_up(100.0 * (1.0 + 0.015), 0.01), 101.0)
        // reentry_price = max(round_up(101.5, 0.01), 101.0)
        // reentry_price = max(101.5, 101.0) = 101.5
        assert_eq!(reentry_price, 101.5);
    }

    #[test]
    fn test_calc_reentry_qty() {
        let (exchange_params, state_params, bot_params, position, _) = setup_test_params();
        let entry_price = 100.0;
        let reentry_qty = calc_reentry_qty(
            entry_price,
            state_params.balance,
            position.size,
            &exchange_params,
            &bot_params,
        );
        // position_size.abs() * entry_grid_double_down_factor = 1.0 * 2.0 = 2.0
        // cost_to_qty(...) part:
        // cost_to_qty(1000.0, 100.0, false, 1.0) = 10.0
        // 10.0 * total_wallet_exposure_limit * entry_initial_qty_pct = 10.0 * 10.0 * 0.01 = 1.0
        // f64::max(2.0, 1.0) = 2.0
        // round_(2.0, 0.001) = 2.0
        // calc_min_entry_qty(100.0, &exchange_params) = 0.01
        // f64::max(0.01, 2.0) = 2.0
        assert_eq!(reentry_qty, 2.0);
    }

    #[test]
    fn test_calc_cropped_reentry_qty_not_cropped() {
        let (exchange_params, state_params, bot_params, position, _) = setup_test_params();
        let wallet_exposure = 1.0;
        let balance = state_params.balance;
        let entry_qty = 2.0;
        let entry_price = 98.0;

        let (_, cropped_qty) = calc_cropped_reentry_qty(
            &exchange_params,
            &bot_params,
            &position,
            wallet_exposure,
            balance,
            entry_qty,
            entry_price,
        );

        assert_eq!(cropped_qty, 2.0);
    }

    #[test]
    fn test_calc_cropped_reentry_qty_is_cropped() {
        let (exchange_params, state_params, bot_params, position, _) = setup_test_params();
        // position size is 1.0, price is 100.0
        // balance is 1000.0
        // total_wallet_exposure_limit is 10.0
        let wallet_exposure = calc_wallet_exposure(
            exchange_params.c_mult,
            state_params.balance,
            position.size,
            position.price,
            exchange_params.inverse,
        );
        let balance = state_params.balance;
        let entry_qty = 120.0;
        let entry_price = 90.0;
        // wallet_exposure_if_filled becomes 10.9, which is > 10.0 * 1.01

        let (_, cropped_qty) = calc_cropped_reentry_qty(
            &exchange_params,
            &bot_params,
            &position,
            wallet_exposure,
            balance,
            entry_qty,
            entry_price,
        );

        // interpolated qty should be 110.1
        assert_eq!(round_(cropped_qty, exchange_params.qty_step), 110.0);
    }
}
