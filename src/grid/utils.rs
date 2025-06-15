use crate::constants::{LONG, SHORT};
use crate::types::ExchangeParams;
use std::cmp::Ordering;

/// Rounds a number to the specified number of decimal places.
fn round_to_decimal_places(value: f64, decimal_places: usize) -> f64 {
    let multiplier = 10f64.powi(decimal_places as i32);
    (value * multiplier).round() / multiplier
}

/// Rounds a number up to the nearest multiple of a given step size.
///
/// This is typically used for rounding prices up to the exchange's required price tick size.
///
/// # Arguments
///
/// * `n` - The number to round.
/// * `step` - The multiple to round up to (e.g., 0.01 for a price step of 0.01).
///
/// # Returns
///
/// The rounded-up number.
pub fn round_up(n: f64, step: f64) -> f64 {
    let result = (n / step).ceil() * step;
    round_to_decimal_places(result, 10)
}

/// Rounds a number to the nearest multiple of a given step size.
///
/// This is used for rounding quantities or prices to the nearest valid increment.
///
/// # Arguments
///
/// * `n` - The number to round.
/// * `step` - The multiple to round to (e.g., 0.001 for a quantity step).
///
/// # Returns
///
/// The rounded number.
pub fn round_(n: f64, step: f64) -> f64 {
    let result = (n / step).round() * step;
    round_to_decimal_places(result, 10)
}

/// Rounds a number down to the nearest multiple of a given step size.
///
/// This is typically used for rounding prices down to the exchange's required price tick size.
///
/// # Arguments
///
/// * `n` - The number to round.
/// * `step` - The multiple to round down to (e.g., 0.01 for a price step of 0.01).
///
/// # Returns
///
/// The rounded-down number.
pub fn round_dn(n: f64, step: f64) -> f64 {
    let result = (n / step).floor() * step;
    round_to_decimal_places(result, 10)
}

/// Rounds a number to a dynamic number of significant digits.
///
/// # Arguments
///
/// * `n` - The number to round.
/// * `d` - The number of significant digits.
///
/// # Returns
///
/// The number rounded to `d` significant digits.
pub fn round_dynamic(n: f64, d: i32) -> f64 {
    if n == 0.0 {
        return n;
    }
    let shift = d - (n.abs().log10().floor() as i32) - 1;
    let multiplier = 10f64.powi(shift);
    let result = (n * multiplier).round() / multiplier;
    round_to_decimal_places(result, 10)
}

/// Rounds a number up to a dynamic number of significant digits.
///
/// # Arguments
///
/// * `n` - The number to round.
/// * `d` - The number of significant digits.
///
/// # Returns
///
/// The number rounded up to `d` significant digits.
pub fn round_dynamic_up(n: f64, d: i32) -> f64 {
    if n == 0.0 {
        return n;
    }
    let shift = d - (n.abs().log10().floor() as i32) - 1;
    let multiplier = 10f64.powi(shift);
    let result = (n * multiplier).ceil() / multiplier;
    round_to_decimal_places(result, 10)
}

/// Rounds a number down to a dynamic number of significant digits.
///
/// # Arguments
///
/// * `n` - The number to round.
/// * `d` - The number of significant digits.
///
/// # Returns
///
/// The number rounded down to `d` significant digits.
pub fn round_dynamic_dn(n: f64, d: i32) -> f64 {
    if n == 0.0 {
        return n;
    }
    let shift = d - (n.abs().log10().floor() as i32) - 1;
    let multiplier = 10f64.powi(shift);
    let result = (n * multiplier).floor() / multiplier;
    round_to_decimal_places(result, 10)
}

/// Calculates the absolute percentage difference between two numbers.
///
/// # Arguments
///
/// * `x` - The first number.
/// * `y` - The second number (the reference).
///
/// # Returns
///
/// The absolute difference `|x - y| / |y|`. Returns `Infinity` if `y` is 0 and `x` is not.
pub fn calc_diff(x: f64, y: f64) -> f64 {
    if y == 0.0 {
        if x == 0.0 {
            0.0
        } else {
            f64::INFINITY
        }
    } else {
        (x - y).abs() / y.abs()
    }
}

/// Converts a given cost into a quantity based on price.
///
/// # Arguments
///
/// * `cost` - The total cost (in quote currency).
/// * `price` - The price per unit.
/// * `inverse` - `true` for inverse contracts, `false` for linear.
/// * `c_mult` - The contract multiplier.
///
/// # Returns
///
/// The calculated quantity (in base currency).
pub fn cost_to_qty(cost: f64, price: f64, inverse: bool, c_mult: f64) -> f64 {
    if inverse {
        (cost * price) / c_mult
    } else if price > 0.0 {
        (cost / price) / c_mult
    } else {
        0.0
    }
}

/// Converts a given quantity into a cost based on price.
///
/// # Arguments
///
/// * `qty` - The quantity (in base currency).
/// * `price` - The price per unit.
/// * `inverse` - `true` for inverse contracts, `false` for linear.
/// * `c_mult` - The contract multiplier.
///
/// # Returns
///
/// The calculated cost (in quote currency).
pub fn qty_to_cost(qty: f64, price: f64, inverse: bool, c_mult: f64) -> f64 {
    if inverse {
        if price > 0.0 {
            (qty.abs() / price) * c_mult
        } else {
            0.0
        }
    } else {
        (qty.abs() * price) * c_mult
    }
}

/// Calculates the current wallet exposure as a ratio of position cost to total balance.
///
/// # Arguments
///
/// * `c_mult` - The contract multiplier.
/// * `balance` - The current wallet balance.
/// * `position_size` - The size of the current position.
/// * `position_price` - The average entry price of the current position.
/// * `inverse` - `true` for inverse contracts, `false` for linear.
///
/// # Returns
///
/// The wallet exposure as a decimal (e.g., 0.5 for 50%).
pub fn calc_wallet_exposure(
    c_mult: f64, balance: f64, position_size: f64, position_price: f64, inverse: bool,
) -> f64 {
    if balance <= 0.0 || position_size == 0.0 {
        return 0.0;
    }
    qty_to_cost(position_size, position_price, inverse, c_mult) / balance
}

/// Calculates the hypothetical wallet exposure if a new order were to be filled.
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `psize` - Current position size.
/// * `pprice` - Current position average price.
/// * `qty` - The quantity of the order to be filled.
/// * `price` - The price at which the order would fill.
/// * `inverse` - `true` for inverse contracts.
/// * `exchange_params` - General parameters for the exchange.
///
/// # Returns
///
/// The hypothetical wallet exposure after the fill.
pub fn calc_wallet_exposure_if_filled(
    balance: f64, psize: f64, pprice: f64, qty: f64, price: f64, inverse: bool,
    exchange_params: &ExchangeParams,
) -> f64 {
    let psize = round_(psize.abs(), exchange_params.qty_step);
    let qty = round_(qty.abs(), exchange_params.qty_step);
    let (new_psize, new_pprice) =
        calc_new_psize_pprice(psize, pprice, qty, price, exchange_params.qty_step);
    calc_wallet_exposure(
        exchange_params.c_mult,
        balance,
        new_psize,
        new_pprice,
        inverse,
    )
}

/// Calculates the new position size and average price after a trade.
///
/// # Arguments
///
/// * `psize` - The current position size.
/// * `pprice` - The current average position price.
/// * `qty` - The quantity of the new trade.
/// * `price` - The price of the new trade.
/// * `qty_step` - The quantity step for rounding.
///
/// # Returns
///
/// A tuple containing the new position size and new average price.
pub fn calc_new_psize_pprice(
    psize: f64, pprice: f64, qty: f64, price: f64, qty_step: f64,
) -> (f64, f64) {
    if qty == 0.0 {
        return (psize, pprice);
    }
    if psize == 0.0 {
        return (qty, price);
    }
    let new_psize = round_(psize + qty, qty_step);
    if new_psize == 0.0 {
        return (0.0, 0.0);
    }
    (
        new_psize,
        nan_to_0(pprice) * (psize / new_psize) + price * (qty / new_psize),
    )
}

/// Replaces `NaN` with 0.0.
fn nan_to_0(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value
    }
}

/// Performs Lagrange interpolation to find a `y` value for a given `x`.
///
/// # Arguments
///
/// * `x` - The point at which to evaluate the interpolated function.
/// * `xs` - A slice of known x-coordinates.
/// * `ys` - A slice of known y-coordinates.
///
/// # Returns
///
/// The interpolated `y` value at point `x`.
pub fn interpolate(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    assert_eq!(xs.len(), ys.len(), "xs and ys must have the same length");

    let n = xs.len();
    let mut result = 0.0;

    for i in 0..n {
        let mut term = ys[i];
        for j in 0..n {
            if i != j {
                term *= (x - xs[j]) / (xs[i] - xs[j]);
            }
        }
        result += term;
    }

    result
}

/// Calculates the Profit and Loss (PNL) for a long position.
///
/// # Arguments
///
/// * `entry_price` - The average entry price.
/// * `close_price` - The price at which the position is closed.
/// * `qty` - The quantity of the position.
/// * `inverse` - `true` for inverse contracts.
/// * `c_mult` - The contract multiplier.
///
/// # Returns
///
/// The calculated PNL.
pub fn calc_pnl_long(
    entry_price: f64, close_price: f64, qty: f64, inverse: bool, c_mult: f64,
) -> f64 {
    if inverse {
        if entry_price == 0.0 || close_price == 0.0 {
            0.0
        } else {
            qty.abs() * c_mult * (1.0 / entry_price - 1.0 / close_price)
        }
    } else {
        qty.abs() * c_mult * (close_price - entry_price)
    }
}

/// Calculates the Profit and Loss (PNL) for a short position.
///
/// # Arguments
///
/// * `entry_price` - The average entry price.
/// * `close_price` - The price at which the position is closed.
/// * `qty` - The quantity of the position.
/// * `inverse` - `true` for inverse contracts.
/// * `c_mult` - The contract multiplier.
///
/// # Returns
///
/// The calculated PNL.
pub fn calc_pnl_short(
    entry_price: f64, close_price: f64, qty: f64, inverse: bool, c_mult: f64,
) -> f64 {
    if inverse {
        if entry_price == 0.0 || close_price == 0.0 {
            0.0
        } else {
            qty.abs() * c_mult * (1.0 / close_price - 1.0 / entry_price)
        }
    } else {
        qty.abs() * c_mult * (entry_price - close_price)
    }
}

/// Calculates the percentage difference between the current price and the position's average price.
///
/// # Arguments
///
/// * `pside` - The side of the position (`LONG` or `SHORT`).
/// * `pprice` - The average price of the position.
/// * `price` - The current price to compare against.
///
/// # Returns
///
/// The percentage difference. Positive for profit, negative for loss.
pub fn calc_pprice_diff_int(pside: usize, pprice: f64, price: f64) -> f64 {
    match pside {
        LONG => {
            // long
            if pprice > 0.0 {
                1.0 - price / pprice
            } else {
                0.0
            }
        }
        SHORT => {
            // short
            if pprice > 0.0 {
                price / pprice - 1.0
            } else {
                0.0
            }
        }
        _ => panic!("unknown pside {}", pside),
    }
}

/// Calculates the amount of funds available for "auto unstuck" orders based on historical performance.
/// It allows spending a certain percentage of the balance, plus any unrealized drop from the peak balance.
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `loss_allowance_pct` - The percentage of the balance allowed to be used for unstuck losses.
/// * `pnl_cumsum_max` - The historical maximum cumulative PNL.
/// * `pnl_cumsum_last` - The last recorded cumulative PNL.
///
/// # Returns
///
/// The calculated cost allowance for auto-unstuck trades.
pub fn calc_auto_unstuck_allowance(
    balance: f64, loss_allowance_pct: f64, pnl_cumsum_max: f64, pnl_cumsum_last: f64,
) -> f64 {
    // allow up to x% drop from balance peak for auto unstuck

    let balance_peak = balance + (pnl_cumsum_max - pnl_cumsum_last);
    let drop_since_peak_pct = balance / balance_peak - 1.0;
    (balance_peak * (loss_allowance_pct + drop_since_peak_pct)).max(0.0)
}

/// Determines the entry price based on the lower EMA band, adjusted by a distance factor.
/// The price is capped at the current best bid from the order book.
///
/// # Arguments
///
/// * `price_step` - The minimum price increment for the market.
/// * `order_book_bid` - The best bid price from the order book.
/// * `ema_bands_lower` - The value of the lower EMA band.
/// * `ema_dist` - The distance from the EMA band to place the order, as a decimal.
///
/// # Returns
///
/// The calculated bid price for an EMA-based entry.
pub fn calc_ema_price_bid(
    price_step: f64, order_book_bid: f64, ema_bands_lower: f64, ema_dist: f64,
) -> f64 {
    f64::min(
        order_book_bid,
        round_dn(ema_bands_lower * (1.0 - ema_dist), price_step),
    )
}

/// Determines the entry price based on the upper EMA band, adjusted by a distance factor.
/// The price is floored at the current best ask from the order book.
///
/// # Arguments
///
/// * `price_step` - The minimum price increment for the market.
/// * `order_book_ask` - The best ask price from the order book.
/// * `ema_bands_upper` - The value of the upper EMA band.
/// * `ema_dist` - The distance from the EMA band to place the order, as a decimal.
///
/// # Returns
///
/// The calculated ask price for an EMA-based entry.
pub fn calc_ema_price_ask(
    price_step: f64, order_book_ask: f64, ema_bands_upper: f64, ema_dist: f64,
) -> f64 {
    f64::max(
        order_book_ask,
        round_up(ema_bands_upper * (1.0 + ema_dist), price_step),
    )
}

/// Calculates the Exponential Moving Average (EMA).
///
/// # Arguments
///
/// * `prev_ema` - The previous EMA value.
/// * `price` - The current price.
/// * `span` - The lookback period for the EMA.
///
/// # Returns
///
/// The new EMA value.
pub fn calc_ema(prev_ema: f64, price: f64, span: f64) -> f64 {
    let multiplier = 2.0 / (span + 1.0);
    (price * multiplier) + (prev_ema * (1.0 - multiplier))
}

/// Calculates the minimum entry quantity, considering both exchange minimums and cost minimums.
///
/// # Arguments
///
/// * `price` - The entry price.
/// * `inverse` - `true` for inverse contracts.
/// * `c_mult` - The contract multiplier.
/// * `qty_step` - The minimum quantity increment.
/// * `min_qty` - The exchange's minimum order quantity.
/// * `min_cost` - The exchange's minimum order cost.
///
/// # Returns
///
/// The minimum allowed entry quantity.
pub fn calc_min_entry_qty(
    price: f64, inverse: bool, c_mult: f64, qty_step: f64, min_qty: f64, min_cost: f64,
) -> f64 {
    if inverse {
        min_qty
    } else {
        f64::max(
            min_qty,
            round_up(cost_to_qty(min_cost, price, inverse, c_mult), qty_step),
        )
    }
}

/// Calculates the total equity of the account, including unrealized PNL from open positions.
///
/// # Arguments
///
/// * `balance` - The current wallet balance.
/// * `psize_long` - The size of the long position.
/// * `pprice_long` - The average price of the long position.
/// * `psize_short` - The size of the short position.
/// * `pprice_short` - The average price of the short position.
/// * `last_price` - The current market price.
/// * `inverse` - `true` for inverse contracts.
/// * `c_mult` - The contract multiplier.
///
/// # Returns
///
/// The total account equity.
pub fn calc_equity(
    balance: f64, psize_long: f64, pprice_long: f64, psize_short: f64, pprice_short: f64,
    last_price: f64, inverse: bool, c_mult: f64,
) -> f64 {
    let mut equity = balance;
    if pprice_long != 0.0 && psize_long != 0.0 {
        equity += calc_pnl_long(pprice_long, last_price, psize_long, inverse, c_mult);
    }
    if pprice_short != 0.0 && psize_short != 0.0 {
        equity += calc_pnl_short(pprice_short, last_price, psize_short, inverse, c_mult);
    }
    equity
}
/// Calculates the quantity for the initial entry order.
///
/// This is based on a percentage of the wallet exposure limit, ensuring it also meets
/// the minimum order size requirements.
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `initial_entry_price` - The price for the initial entry.
/// * `inverse` - `true` for inverse contracts.
/// * `qty_step` - Minimum quantity increment.
/// * `min_qty` - Exchange minimum quantity.
/// * `min_cost` - Exchange minimum cost.
/// * `c_mult` - Contract multiplier.
/// * `wallet_exposure_limit` - The maximum desired wallet exposure.
/// * `initial_qty_pct` - The percentage of the wallet exposure limit to use for the initial entry.
///
/// # Returns
///
/// The calculated quantity for the initial entry order.
pub fn calc_initial_entry_qty(
    balance: f64, initial_entry_price: f64, inverse: bool, qty_step: f64, min_qty: f64,
    min_cost: f64, c_mult: f64, wallet_exposure_limit: f64, initial_qty_pct: f64,
) -> f64 {
    f64::max(
        calc_min_entry_qty(
            initial_entry_price,
            inverse,
            c_mult,
            qty_step,
            min_qty,
            min_cost,
        ),
        round_(
            cost_to_qty(
                balance * wallet_exposure_limit * initial_qty_pct,
                initial_entry_price,
                inverse,
                c_mult,
            ),
            qty_step,
        ),
    )
}

/// Finds the entry order quantity that will bring the wallet exposure to a target value.
///
/// This function uses an iterative solver with interpolation to find the quantity.
/// It starts with initial guesses and refines them to converge on the target exposure.
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `psize` - Current position size.
/// * `pprice` - Current position price.
/// * `wallet_exposure_target` - The desired wallet exposure after the entry.
/// * `entry_price` - The price of the potential entry order.
/// * `inverse` - `true` for inverse contracts.
/// * `exchange_params` - General parameters for the exchange.
///
/// # Returns
///
/// The calculated entry quantity to reach the target exposure. Returns 0.0 if the
/// current exposure is already near or above the target.
pub fn find_entry_qty_bringing_wallet_exposure_to_target(
    balance: f64, psize: f64, pprice: f64, wallet_exposure_target: f64, entry_price: f64,
    inverse: bool, exchange_params: &ExchangeParams,
) -> f64 {
    if wallet_exposure_target == 0.0 {
        return 0.0;
    }
    let wallet_exposure =
        calc_wallet_exposure(exchange_params.c_mult, balance, psize, pprice, inverse);
    if wallet_exposure >= wallet_exposure_target * 0.99 {
        return 0.0;
    }

    let mut guesses = Vec::new();
    let mut vals = Vec::new();
    let mut evals = Vec::new();

    guesses.push(round_(
        psize.abs() * wallet_exposure_target / wallet_exposure.max(0.01),
        exchange_params.qty_step,
    ));
    vals.push(calc_wallet_exposure_if_filled(
        balance,
        psize,
        pprice,
        guesses[0],
        entry_price,
        inverse,
        exchange_params,
    ));
    evals.push((vals[0] - wallet_exposure_target).abs() / wallet_exposure_target);

    guesses.push(
        (guesses[0] * 1.2)
            .max(guesses[0] + exchange_params.qty_step)
            .max(0.0),
    );
    vals.push(calc_wallet_exposure_if_filled(
        balance,
        psize,
        pprice,
        guesses[1],
        entry_price,
        inverse,
        exchange_params,
    ));
    evals.push((vals[1] - wallet_exposure_target).abs() / wallet_exposure_target);

    for _ in 0..15 {
        if guesses.last() == guesses.get(guesses.len() - 2) {
            let last_guess = guesses.last().unwrap().clone();
            guesses.push((last_guess * 1.1).max(last_guess + exchange_params.qty_step));
            vals.push(calc_wallet_exposure_if_filled(
                balance,
                psize,
                pprice,
                *guesses.last().unwrap(),
                entry_price,
                inverse,
                exchange_params,
            ));
        }
        let new_guess = interpolate(
            wallet_exposure_target,
            &vals[vals.len() - 2..],
            &guesses[guesses.len() - 2..],
        )
        .max(0.0);
        let new_guess = round_(new_guess, exchange_params.qty_step);
        guesses.push(new_guess);
        vals.push(calc_wallet_exposure_if_filled(
            balance,
            psize,
            pprice,
            new_guess,
            entry_price,
            inverse,
            exchange_params,
        ));
        evals.push((vals.last().unwrap() - wallet_exposure_target).abs() / wallet_exposure_target);

        if *evals.last().unwrap() < 0.01 {
            break;
        }
    }

    let mut evals_guesses: Vec<_> = evals.iter().zip(guesses.iter()).collect();
    evals_guesses.sort_by(|a, b| a.0.partial_cmp(b.0).unwrap_or(Ordering::Equal));
    *evals_guesses[0].1
}

/// Finds the close order quantity for a long position that will bring wallet exposure to a target.
///
/// This function uses an iterative solver to find the portion of the position to close.
/// It's used for partially closing a position to reduce risk (de-risking).
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `psize` - Current position size.
/// * `pprice` - Current position price.
/// * `wallet_exposure_target` - The desired wallet exposure after the close.
/// * `close_price` - The price at which the position would be closed.
/// * `inverse` - `true` for inverse contracts.
/// * `exchange_params` - General parameters for the exchange.
///
/// # Returns
///
/// The calculated close quantity. Returns 0.0 if exposure is already below the target,
/// or the full position size if the target is 0.
pub fn find_close_qty_long_bringing_wallet_exposure_to_target(
    balance: f64, psize: f64, pprice: f64, wallet_exposure_target: f64, close_price: f64,
    inverse: bool, exchange_params: &ExchangeParams,
) -> f64 {
    let eval = |guess: f64| {
        let pnl = calc_pnl_long(pprice, close_price, guess, inverse, exchange_params.c_mult);
        let new_balance = balance + pnl;
        qty_to_cost(psize - guess, pprice, inverse, exchange_params.c_mult) / new_balance
    };

    if wallet_exposure_target == 0.0 {
        return psize;
    }
    let wallet_exposure =
        calc_wallet_exposure(exchange_params.c_mult, balance, psize, pprice, inverse);
    if wallet_exposure <= wallet_exposure_target * 1.001 {
        return 0.0;
    }

    let mut guesses = Vec::new();
    let mut vals = Vec::new();
    let mut evals = Vec::new();

    guesses.push(
        round_(
            psize * (1.0 - wallet_exposure_target / wallet_exposure),
            exchange_params.qty_step,
        )
        .max(0.0)
        .min(psize),
    );
    vals.push(eval(guesses[0]));
    evals.push((vals[0] - wallet_exposure_target).abs() / wallet_exposure_target);

    let mut next_guess = (guesses[0] * 1.2).max(guesses[0] + exchange_params.qty_step);
    if next_guess == guesses[0] {
        next_guess = (guesses[0] * 0.8).min(guesses[0] - exchange_params.qty_step);
    }
    guesses.push(next_guess.max(0.0).min(psize));
    vals.push(eval(guesses[1]));
    evals.push((vals[1] - wallet_exposure_target).abs() / wallet_exposure_target);

    for _ in 0..15 {
        let egv: Vec<_> = evals
            .iter()
            .zip(guesses.iter())
            .zip(vals.iter())
            .map(|((e, g), v)| (*e, *g, *v))
            .collect();
        let mut sorted_egv = egv;
        sorted_egv.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

        let new_guess = interpolate(
            wallet_exposure_target,
            &[sorted_egv[0].2, sorted_egv[1].2],
            &[sorted_egv[0].1, sorted_egv[1].1],
        );

        let mut new_guess = round_(new_guess, exchange_params.qty_step)
            .max(0.0)
            .min(psize);

        if guesses.contains(&new_guess) {
            new_guess = (new_guess - exchange_params.qty_step).max(0.0).min(psize);
            if guesses.contains(&new_guess) {
                new_guess = (new_guess + 2.0 * exchange_params.qty_step)
                    .max(0.0)
                    .min(psize);
                if guesses.contains(&new_guess) {
                    break;
                }
            }
        }

        guesses.push(new_guess);
        vals.push(eval(new_guess));
        evals.push((vals.last().unwrap() - wallet_exposure_target).abs() / wallet_exposure_target);

        if *evals.last().unwrap() < 0.01 {
            break;
        }
    }

    let mut evals_guesses: Vec<_> = evals.iter().zip(guesses.iter()).collect();
    evals_guesses.sort_by(|a, b| a.0.partial_cmp(b.0).unwrap_or(Ordering::Equal));
    *evals_guesses[0].1
}

/// Finds the close order quantity for a short position that will bring wallet exposure to a target.
///
/// This function uses an iterative solver to find the portion of the position to close.
/// It's used for partially closing a position to reduce risk (de-risking).
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `psize` - Current position size (negative for short).
/// * `pprice` - Current position price.
/// * `wallet_exposure_target` - The desired wallet exposure after the close.
/// * `close_price` - The price at which the position would be closed.
/// * `inverse` - `true` for inverse contracts.
/// * `exchange_params` - General parameters for the exchange.
///
/// # Returns
///
/// The calculated close quantity (as a positive value). Returns 0.0 if exposure is
/// already below the target, or the full position size if the target is 0.
pub fn find_close_qty_short_bringing_wallet_exposure_to_target(
    balance: f64, psize: f64, pprice: f64, wallet_exposure_target: f64, close_price: f64,
    inverse: bool, exchange_params: &ExchangeParams,
) -> f64 {
    let eval = |guess: f64| {
        let pnl = calc_pnl_short(pprice, close_price, guess, inverse, exchange_params.c_mult);
        let new_balance = balance + pnl;
        qty_to_cost(psize.abs() - guess, pprice, inverse, exchange_params.c_mult) / new_balance
    };

    if wallet_exposure_target == 0.0 {
        return psize.abs();
    }
    let wallet_exposure =
        calc_wallet_exposure(exchange_params.c_mult, balance, psize, pprice, inverse);
    if wallet_exposure <= wallet_exposure_target * 1.001 {
        return 0.0;
    }

    let mut guesses = Vec::new();
    let mut vals = Vec::new();
    let mut evals = Vec::new();

    let abs_psize = psize.abs();

    guesses.push(
        round_(
            abs_psize * (1.0 - wallet_exposure_target / wallet_exposure),
            exchange_params.qty_step,
        )
        .max(0.0)
        .min(abs_psize),
    );
    vals.push(eval(guesses[0]));
    evals.push((vals[0] - wallet_exposure_target).abs() / wallet_exposure_target);

    let mut next_guess = (guesses[0] * 1.2).max(guesses[0] + exchange_params.qty_step);
    if next_guess == guesses[0] {
        next_guess = (guesses[0] * 0.8).min(guesses[0] - exchange_params.qty_step);
    }
    guesses.push(next_guess.max(0.0).min(abs_psize));
    vals.push(eval(guesses[1]));
    evals.push((vals[1] - wallet_exposure_target).abs() / wallet_exposure_target);

    for _ in 0..15 {
        let egv: Vec<_> = evals
            .iter()
            .zip(guesses.iter())
            .zip(vals.iter())
            .map(|((e, g), v)| (*e, *g, *v))
            .collect();
        let mut sorted_egv = egv;
        sorted_egv.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

        let new_guess = interpolate(
            wallet_exposure_target,
            &[sorted_egv[0].2, sorted_egv[1].2],
            &[sorted_egv[0].1, sorted_egv[1].1],
        );

        let mut new_guess = round_(new_guess, exchange_params.qty_step)
            .max(0.0)
            .min(abs_psize);

        if guesses.contains(&new_guess) {
            new_guess = (new_guess - exchange_params.qty_step)
                .max(0.0)
                .min(abs_psize);
            if guesses.contains(&new_guess) {
                new_guess = (new_guess + 2.0 * exchange_params.qty_step)
                    .max(0.0)
                    .min(abs_psize);
                if guesses.contains(&new_guess) {
                    break;
                }
            }
        }

        guesses.push(new_guess);
        vals.push(eval(new_guess));
        evals.push((vals.last().unwrap() - wallet_exposure_target).abs() / wallet_exposure_target);

        if *evals.last().unwrap() < 0.01 {
            break;
        }
    }

    let mut evals_guesses: Vec<_> = evals.iter().zip(guesses.iter()).collect();
    evals_guesses.sort_by(|a, b| a.0.partial_cmp(b.0).unwrap_or(Ordering::Equal));
    *evals_guesses[0].1
}

/// Calculates the theoretical bankruptcy price for the current combined positions.
///
/// This is the price at which the account equity would drop to zero.
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `psize_long` - Size of the long position.
/// * `pprice_long` - Average price of the long position.
/// * `psize_short` - Size of the short position.
/// * `pprice_short` - Average price of the short position.
/// * `inverse` - `true` for inverse contracts.
/// * `c_mult` - Contract multiplier.
///
/// # Returns
///
/// The calculated bankruptcy price. Returns 0.0 if calculation is not possible.
pub fn calc_bankruptcy_price(
    balance: f64, psize_long: f64, pprice_long: f64, psize_short: f64, pprice_short: f64,
    inverse: bool, c_mult: f64,
) -> f64 {
    let pprice_long = nan_to_0(pprice_long);
    let pprice_short = nan_to_0(pprice_short);
    let psize_long = psize_long * c_mult;
    let abs_psize_short = psize_short.abs() * c_mult;

    let bankruptcy_price = if inverse {
        let short_cost = if pprice_short > 0.0 {
            abs_psize_short / pprice_short
        } else {
            0.0
        };
        let long_cost = if pprice_long > 0.0 {
            psize_long / pprice_long
        } else {
            0.0
        };
        let denominator = short_cost - long_cost - balance;
        if denominator == 0.0 {
            0.0
        } else {
            (abs_psize_short - psize_long) / denominator
        }
    } else {
        let denominator = psize_long - abs_psize_short;
        if denominator == 0.0 {
            0.0
        } else {
            (-balance + psize_long * pprice_long - abs_psize_short * pprice_short) / denominator
        }
    };
    bankruptcy_price.max(0.0)
}
/// Calculates the order quantity for the "clock" mode.
///
/// This mode places orders at regular time intervals. The quantity is based on a percentage
/// of the wallet exposure limit, and can be scaled by the current wallet exposure.
///
/// # Arguments
///
/// * `balance` - Current wallet balance.
/// * `wallet_exposure` - Current wallet exposure.
/// * `entry_price` - The price of the entry order.
/// * `inverse` - `true` for inverse contracts.
/// * `qty_step`...`min_cost` - Standard exchange parameters.
/// * `c_mult` - Contract multiplier.
/// * `qty_pct` - The base quantity as a percentage of the wallet exposure limit.
/// * `we_multiplier` - A multiplier to scale the quantity based on current exposure.
/// * `wallet_exposure_limit` - The maximum desired wallet exposure.
///
/// # Returns
///
/// The calculated order quantity for the clock mode.
pub fn calc_clock_qty(
    balance: f64, wallet_exposure: f64, entry_price: f64, inverse: bool, qty_step: f64,
    min_qty: f64, min_cost: f64, c_mult: f64, qty_pct: f64, we_multiplier: f64,
    wallet_exposure_limit: f64,
) -> f64 {
    let ratio = wallet_exposure / wallet_exposure_limit;
    let cost = balance * wallet_exposure_limit * qty_pct * (1.0 + ratio * we_multiplier);
    f64::max(
        calc_min_entry_qty(entry_price, inverse, c_mult, qty_step, min_qty, min_cost),
        round_(cost_to_qty(cost, entry_price, inverse, c_mult), qty_step),
    )
}

/// Determines if an "auto unstuck" close order should be placed for a long position, and calculates it.
///
/// This logic triggers when wallet exposure exceeds a certain threshold, placing a closing
/// order to reduce the position size and "unstick" the bot from a losing trade.
///
/// # Returns
///
/// A tuple `(quantity, price, label)`.
/// `quantity` is negative for a close order.
/// Returns `(0.0, 0.0, ...)` if no unstuck order is needed.
pub fn calc_auto_unstuck_close_long(
    balance: f64, psize: f64, pprice: f64, lowest_ask: f64, ema_band_upper: f64, utc_now_ms: f64,
    prev_au_fill_ts_close: f64, inverse: bool, qty_step: f64, price_step: f64, min_qty: f64,
    min_cost: f64, c_mult: f64, wallet_exposure_limit: f64,
    auto_unstuck_wallet_exposure_threshold: f64, auto_unstuck_ema_dist: f64,
    auto_unstuck_delay_minutes: f64, auto_unstuck_qty_pct: f64, lowest_normal_close_price: f64,
) -> (f64, f64, &'static str) {
    let threshold = wallet_exposure_limit * (1.0 - auto_unstuck_wallet_exposure_threshold);
    let wallet_exposure = qty_to_cost(psize, pprice, inverse, c_mult) / balance;
    if wallet_exposure > threshold {
        let mut unstuck_close_qty = 0.0;
        let unstuck_close_price = f64::max(
            lowest_ask,
            round_up(ema_band_upper * (1.0 + auto_unstuck_ema_dist), price_step),
        );
        if unstuck_close_price < lowest_normal_close_price {
            if auto_unstuck_delay_minutes != 0.0 && auto_unstuck_qty_pct != 0.0 {
                let delay = calc_delay_between_fills_ms_ask(
                    pprice,
                    lowest_ask,
                    auto_unstuck_delay_minutes * 60.0 * 1000.0,
                    0.0,
                );
                if utc_now_ms - prev_au_fill_ts_close > delay {
                    unstuck_close_qty = psize.min(calc_clock_qty(
                        balance,
                        wallet_exposure,
                        unstuck_close_price,
                        inverse,
                        qty_step,
                        min_qty,
                        min_cost,
                        c_mult,
                        auto_unstuck_qty_pct,
                        0.0,
                        wallet_exposure_limit,
                    ));
                }
            } else {
                // legacy AU mode
                // Note: ExchangeParams are being faked here for now
                let dummy_exchange_params = ExchangeParams {
                    qty_step,
                    price_step,
                    min_qty,
                    min_cost,
                    c_mult,
                    inverse: false,
                };
                unstuck_close_qty = find_close_qty_long_bringing_wallet_exposure_to_target(
                    balance,
                    psize,
                    pprice,
                    threshold * 1.01,
                    unstuck_close_price,
                    inverse,
                    &dummy_exchange_params,
                );
            }
        }
        if unstuck_close_qty != 0.0 {
            let min_entry_qty = calc_min_entry_qty(
                unstuck_close_price,
                inverse,
                c_mult,
                qty_step,
                min_qty,
                min_cost,
            );
            unstuck_close_qty = unstuck_close_qty.max(min_entry_qty);
            return (
                -unstuck_close_qty,
                unstuck_close_price,
                "unstuck_close_long",
            );
        }
    }
    (0.0, 0.0, "unstuck_close_long")
}

/// Calculates a dynamic delay for placing subsequent orders based on price movement.
/// This is used for ask-side orders (long closes, short entries).
///
/// # Arguments
///
/// * `pprice` - The average position price.
/// * `price` - The current market price.
/// * `delay_between_fills_ms` - The base delay in milliseconds.
/// * `delay_weight` - A factor to scale the delay based on the price difference.
///
/// # Returns
///
/// The calculated delay in milliseconds, with a minimum of 60,000ms (1 minute).
#[inline]
pub fn calc_delay_between_fills_ms_ask(
    pprice: f64, price: f64, delay_between_fills_ms: f64, delay_weight: f64,
) -> f64 {
    let pprice_diff = if pprice > 0.0 {
        price / pprice - 1.0
    } else {
        0.0
    };
    f64::max(
        60000.0,
        delay_between_fills_ms * (1.0 - pprice_diff * delay_weight).min(1.0),
    )
}

/// Generates a series of raw, unrounded close prices for a grid.
fn generate_raw_close_prices(
    pprice: f64, min_markup: f64, markup_range: f64, n_close_orders: i32, side: usize,
) -> Vec<f64> {
    let minm = if side == LONG {
        pprice * (1.0 + min_markup)
    } else {
        pprice * (1.0 - min_markup)
    };

    (0..n_close_orders)
        .map(|i| {
            let price_offset =
                (pprice * markup_range / (n_close_orders as f64 - 1.0).max(1.0)) * i as f64;
            if side == LONG {
                minm + price_offset
            } else {
                minm - price_offset
            }
        })
        .collect()
}

/// Calculates a grid of close orders for a long position using the "frontwards" distribution method.
///
/// In this method, the total quantity is divided equally among the available price levels.
///
/// # Returns
///
/// A `Vec` of tuples, where each tuple represents a close order: `(quantity, price, label)`.
/// Quantity is negative for close orders.
pub fn calc_close_grid_frontwards_long(
    balance: f64, psize: f64, pprice: f64, lowest_ask: f64, ema_band_upper: f64, utc_now_ms: f64,
    prev_au_fill_ts_close: f64, inverse: bool, qty_step: f64, price_step: f64, min_qty: f64,
    min_cost: f64, c_mult: f64, wallet_exposure_limit: f64, min_markup: f64, markup_range: f64,
    n_close_orders: f64, auto_unstuck_wallet_exposure_threshold: f64, auto_unstuck_ema_dist: f64,
    auto_unstuck_delay_minutes: f64, auto_unstuck_qty_pct: f64,
) -> Vec<(f64, f64, &'static str)> {
    let mut psize_ = round_dn(psize, qty_step);
    if psize_ == 0.0 {
        return vec![(0.0, 0.0, "")];
    }
    let n_close_orders = n_close_orders.round() as i32;
    let raw_close_prices =
        generate_raw_close_prices(pprice, min_markup, markup_range, n_close_orders, LONG);

    let mut close_prices = Vec::new();
    for p in raw_close_prices {
        let price = round_up(p, price_step);
        if price >= lowest_ask {
            close_prices.push(price);
        }
    }

    if close_prices.is_empty() {
        return vec![(-psize, lowest_ask, "long_nclose")];
    }

    let mut closes = Vec::new();
    if auto_unstuck_wallet_exposure_threshold != 0.0 {
        let auto_unstuck_close = calc_auto_unstuck_close_long(
            balance,
            psize,
            pprice,
            lowest_ask,
            ema_band_upper,
            utc_now_ms,
            prev_au_fill_ts_close,
            inverse,
            qty_step,
            price_step,
            min_qty,
            min_cost,
            c_mult,
            wallet_exposure_limit,
            auto_unstuck_wallet_exposure_threshold,
            auto_unstuck_ema_dist,
            auto_unstuck_delay_minutes,
            auto_unstuck_qty_pct,
            close_prices[0],
        );
        if auto_unstuck_close.0 != 0.0 {
            psize_ = round_(psize_ - auto_unstuck_close.0.abs(), qty_step);
            let min_entry_qty = calc_min_entry_qty(
                auto_unstuck_close.1,
                inverse,
                c_mult,
                qty_step,
                min_qty,
                min_cost,
            );
            if psize_ < min_entry_qty {
                return vec![(-psize, auto_unstuck_close.1, "unstuck_close_long")];
            }
            closes.push(auto_unstuck_close);
        }
    }

    if close_prices.len() == 1 {
        let min_entry_qty = calc_min_entry_qty(
            close_prices[0],
            inverse,
            c_mult,
            qty_step,
            min_qty,
            min_cost,
        );
        if psize_ >= min_entry_qty {
            closes.push((-psize_, close_prices[0], "long_nclose"));
        }
        return if closes.is_empty() {
            vec![(0.0, 0.0, "")]
        } else {
            closes
        };
    }

    let default_close_qty = round_dn(psize_ / close_prices.len() as f64, qty_step);
    for &price in &close_prices[..close_prices.len() - 1] {
        let min_close_qty = calc_min_entry_qty(price, inverse, c_mult, qty_step, min_qty, min_cost);
        if psize_ < min_close_qty {
            break;
        }
        let close_qty = psize_.min(default_close_qty.max(min_close_qty));
        closes.push((-close_qty, price, "long_nclose"));
        psize_ = round_(psize_ - close_qty, qty_step);
    }

    let last_price = if let Some(price) = close_prices.last() {
        *price
    } else {
        // This case should be handled by the is_empty check above, but for safety:
        return if closes.is_empty() {
            vec![(0.0, 0.0, "")]
        } else {
            closes
        };
    };
    let min_close_qty =
        calc_min_entry_qty(last_price, inverse, c_mult, qty_step, min_qty, min_cost);
    if psize_ >= min_close_qty {
        closes.push((-psize_, last_price, "long_nclose"));
    } else if let Some(last_close) = closes.last_mut() {
        *last_close = (
            -round_(last_close.0.abs() + psize_, qty_step),
            last_close.1,
            last_close.2,
        );
    }

    if closes.is_empty() {
        vec![(0.0, 0.0, "")]
    } else {
        closes
    }
}

/// Calculates a grid of close orders for a long position using the "backwards" distribution method.
///
/// In this method, a fixed quantity is placed at each price level, starting from the furthest
/// price and working backwards. This prioritizes filling the further-away orders.
///
/// # Returns
///
/// A `Vec` of tuples, where each tuple represents a close order: `(quantity, price, label)`.
/// Quantity is negative for close orders.
pub fn calc_close_grid_backwards_long(
    balance: f64, psize: f64, pprice: f64, lowest_ask: f64, ema_band_upper: f64, utc_now_ms: f64,
    prev_au_fill_ts_close: f64, inverse: bool, qty_step: f64, price_step: f64, min_qty: f64,
    min_cost: f64, c_mult: f64, wallet_exposure_limit: f64, min_markup: f64, markup_range: f64,
    n_close_orders: f64, auto_unstuck_wallet_exposure_threshold: f64, auto_unstuck_ema_dist: f64,
    auto_unstuck_delay_minutes: f64, auto_unstuck_qty_pct: f64,
) -> Vec<(f64, f64, &'static str)> {
    let mut psize_ = round_dn(psize, qty_step);
    if psize_ == 0.0 {
        return vec![(0.0, 0.0, "")];
    }
    let full_psize = cost_to_qty(balance * wallet_exposure_limit, pprice, inverse, c_mult);

    let n_close_orders_f = n_close_orders
        .min(full_psize / calc_min_entry_qty(pprice, inverse, c_mult, qty_step, min_qty, min_cost))
        .max(1.0);
    let n_close_orders = n_close_orders_f.round() as i32;

    let raw_close_prices =
        generate_raw_close_prices(pprice, min_markup, markup_range, n_close_orders, LONG);

    let mut close_prices_all = Vec::new();
    let mut close_prices = Vec::new();
    for p in raw_close_prices {
        let price = round_up(p, price_step);
        if !close_prices_all.contains(&price) {
            close_prices_all.push(price);
            if price >= lowest_ask {
                close_prices.push(price);
            }
        }
    }

    if close_prices.is_empty() {
        return vec![(-psize, lowest_ask, "long_nclose")];
    }

    let mut closes = Vec::new();
    if auto_unstuck_wallet_exposure_threshold != 0.0 {
        let auto_unstuck_close = calc_auto_unstuck_close_long(
            balance,
            psize,
            pprice,
            lowest_ask,
            ema_band_upper,
            utc_now_ms,
            prev_au_fill_ts_close,
            inverse,
            qty_step,
            price_step,
            min_qty,
            min_cost,
            c_mult,
            wallet_exposure_limit,
            auto_unstuck_wallet_exposure_threshold,
            auto_unstuck_ema_dist,
            auto_unstuck_delay_minutes,
            auto_unstuck_qty_pct,
            close_prices[0],
        );
        if auto_unstuck_close.0 != 0.0 {
            psize_ = round_(psize_ - auto_unstuck_close.0.abs(), qty_step);
            let min_entry_qty = calc_min_entry_qty(
                auto_unstuck_close.1,
                inverse,
                c_mult,
                qty_step,
                min_qty,
                min_cost,
            );
            if psize_ < min_entry_qty {
                return vec![(-psize, auto_unstuck_close.1, "unstuck_close_long")];
            }
            closes.push(auto_unstuck_close);
        }
    }

    if close_prices.len() == 1 {
        let min_entry_qty = calc_min_entry_qty(
            close_prices[0],
            inverse,
            c_mult,
            qty_step,
            min_qty,
            min_cost,
        );
        if psize_ >= min_entry_qty {
            closes.push((-psize_, close_prices[0], "long_nclose"));
        }
        return if closes.is_empty() {
            vec![(0.0, 0.0, "")]
        } else {
            closes
        };
    }

    let qty_per_close = (full_psize / close_prices_all.len() as f64).max(min_qty);
    let qty_per_close = round_up(qty_per_close, qty_step);

    for &price in close_prices.iter().rev() {
        let min_entry_qty = calc_min_entry_qty(price, inverse, c_mult, qty_step, min_qty, min_cost);
        let qty = psize_.min(qty_per_close.max(min_entry_qty));
        if qty < min_entry_qty {
            if let Some(last_close) = closes.last_mut() {
                *last_close = (
                    -round_(last_close.0.abs() + psize_, qty_step),
                    last_close.1,
                    last_close.2,
                );
            } else {
                closes.push((-psize_, price, "long_nclose"));
            }
            psize_ = 0.0;
            break;
        }
        closes.push((-qty, price, "long_nclose"));
        psize_ = round_(psize_ - qty, qty_step);
        if psize_ <= 0.0 {
            break;
        }
    }

    if psize_ > 0.0 {
        if let Some(last_close) = closes.last_mut() {
            *last_close = (
                -round_(last_close.0.abs() + psize_, qty_step),
                last_close.1,
                last_close.2,
            );
        }
    }

    closes.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
    if closes.is_empty() {
        vec![(0.0, 0.0, "")]
    } else {
        closes
    }
}

/// Determines if an "auto unstuck" close order should be placed for a short position, and calculates it.
///
/// This logic triggers when wallet exposure exceeds a certain threshold, placing a closing
/// order to reduce the position size and "unstick" the bot from a losing trade.
///
/// # Returns
///
/// A tuple `(quantity, price, label)`.
/// `quantity` is positive for a close order.
/// Returns `(0.0, 0.0, ...)` if no unstuck order is needed.
pub fn calc_auto_unstuck_close_short(
    balance: f64, psize: f64, pprice: f64, highest_bid: f64, ema_band_lower: f64, utc_now_ms: f64,
    prev_au_fill_ts_close: f64, inverse: bool, qty_step: f64, price_step: f64, min_qty: f64,
    min_cost: f64, c_mult: f64, wallet_exposure_limit: f64,
    auto_unstuck_wallet_exposure_threshold: f64, auto_unstuck_ema_dist: f64,
    auto_unstuck_delay_minutes: f64, auto_unstuck_qty_pct: f64, highest_normal_close_price: f64,
) -> (f64, f64, &'static str) {
    let threshold = wallet_exposure_limit * (1.0 - auto_unstuck_wallet_exposure_threshold);
    let wallet_exposure = qty_to_cost(psize, pprice, inverse, c_mult) / balance;
    if wallet_exposure > threshold {
        let mut unstuck_close_qty = 0.0;
        let unstuck_close_price = f64::min(
            highest_bid,
            round_dn(ema_band_lower * (1.0 - auto_unstuck_ema_dist), price_step),
        );
        if unstuck_close_price > highest_normal_close_price {
            if auto_unstuck_delay_minutes != 0.0 && auto_unstuck_qty_pct != 0.0 {
                let delay = calc_delay_between_fills_ms_bid(
                    pprice,
                    highest_bid,
                    auto_unstuck_delay_minutes * 60.0 * 1000.0,
                    0.0,
                );
                if utc_now_ms - prev_au_fill_ts_close > delay {
                    unstuck_close_qty = psize.abs().min(calc_clock_qty(
                        balance,
                        wallet_exposure,
                        unstuck_close_price,
                        inverse,
                        qty_step,
                        min_qty,
                        min_cost,
                        c_mult,
                        auto_unstuck_qty_pct,
                        0.0,
                        wallet_exposure_limit,
                    ));
                }
            } else {
                // legacy AU mode
                let dummy_exchange_params = ExchangeParams {
                    qty_step,
                    price_step,
                    min_qty,
                    min_cost,
                    c_mult,
                    inverse: false,
                };
                unstuck_close_qty = find_close_qty_short_bringing_wallet_exposure_to_target(
                    balance,
                    psize,
                    pprice,
                    threshold * 1.01,
                    unstuck_close_price,
                    inverse,
                    &dummy_exchange_params,
                );
            }
        }
        if unstuck_close_qty != 0.0 {
            let min_entry_qty = calc_min_entry_qty(
                unstuck_close_price,
                inverse,
                c_mult,
                qty_step,
                min_qty,
                min_cost,
            );
            unstuck_close_qty = unstuck_close_qty.max(min_entry_qty);
            return (
                unstuck_close_qty,
                unstuck_close_price,
                "unstuck_close_short",
            );
        }
    }
    (0.0, 0.0, "unstuck_close_short")
}

/// Calculates a dynamic delay for placing subsequent orders based on price movement.
/// This is used for bid-side orders (long entries, short closes).
///
/// # Arguments
///
/// * `pprice` - The average position price.
/// * `price` - The current market price.
/// * `delay_between_fills_ms` - The base delay in milliseconds.
/// * `delay_weight` - A factor to scale the delay based on the price difference.
///
/// # Returns
///
/// The calculated delay in milliseconds, with a minimum of 60,000ms (1 minute).
#[inline]
pub fn calc_delay_between_fills_ms_bid(
    pprice: f64, price: f64, delay_between_fills_ms: f64, delay_weight: f64,
) -> f64 {
    let pprice_diff = if pprice > 0.0 {
        1.0 - price / pprice
    } else {
        0.0
    };
    f64::max(
        60000.0,
        delay_between_fills_ms * (1.0 - pprice_diff * delay_weight).min(1.0),
    )
}

/// Calculates a grid of close orders for a short position using the "frontwards" distribution method.
///
/// In this method, the total quantity is divided equally among the available price levels.
///
/// # Returns
///
/// A `Vec` of tuples, where each tuple represents a close order: `(quantity, price, label)`.
/// Quantity is positive for close orders.
pub fn calc_close_grid_frontwards_short(
    balance: f64, psize: f64, pprice: f64, highest_bid: f64, ema_band_lower: f64, utc_now_ms: f64,
    prev_au_fill_ts_close: f64, inverse: bool, qty_step: f64, price_step: f64, min_qty: f64,
    min_cost: f64, c_mult: f64, wallet_exposure_limit: f64, min_markup: f64, markup_range: f64,
    n_close_orders: f64, auto_unstuck_wallet_exposure_threshold: f64, auto_unstuck_ema_dist: f64,
    auto_unstuck_delay_minutes: f64, auto_unstuck_qty_pct: f64,
) -> Vec<(f64, f64, &'static str)> {
    let mut psize_ = round_dn(psize.abs(), qty_step);
    if psize_ == 0.0 {
        return vec![(0.0, 0.0, "")];
    }
    let n_close_orders = n_close_orders.round() as i32;

    let raw_close_prices =
        generate_raw_close_prices(pprice, min_markup, markup_range, n_close_orders, SHORT);

    let mut close_prices = Vec::new();
    for p in raw_close_prices {
        let price = round_dn(p, price_step);
        if price <= highest_bid {
            close_prices.push(price);
        }
    }

    if close_prices.is_empty() {
        return vec![(psize_, highest_bid, "short_nclose")];
    }

    let mut closes = Vec::new();
    if auto_unstuck_wallet_exposure_threshold != 0.0 {
        let auto_unstuck_close = calc_auto_unstuck_close_short(
            balance,
            psize,
            pprice,
            highest_bid,
            ema_band_lower,
            utc_now_ms,
            prev_au_fill_ts_close,
            inverse,
            qty_step,
            price_step,
            min_qty,
            min_cost,
            c_mult,
            wallet_exposure_limit,
            auto_unstuck_wallet_exposure_threshold,
            auto_unstuck_ema_dist,
            auto_unstuck_delay_minutes,
            auto_unstuck_qty_pct,
            close_prices[0],
        );
        if auto_unstuck_close.0 != 0.0 {
            psize_ = round_(psize_ - auto_unstuck_close.0.abs(), qty_step);
            let min_entry_qty = calc_min_entry_qty(
                auto_unstuck_close.1,
                inverse,
                c_mult,
                qty_step,
                min_qty,
                min_cost,
            );
            if psize_ < min_entry_qty {
                return vec![(psize.abs(), auto_unstuck_close.1, "unstuck_close_short")];
            }
            closes.push(auto_unstuck_close);
        }
    }

    if close_prices.len() == 1 {
        let min_entry_qty = calc_min_entry_qty(
            close_prices[0],
            inverse,
            c_mult,
            qty_step,
            min_qty,
            min_cost,
        );
        if psize_ >= min_entry_qty {
            closes.push((psize_, close_prices[0], "short_nclose"));
        }
        return if closes.is_empty() {
            vec![(0.0, 0.0, "")]
        } else {
            closes
        };
    }

    let default_close_qty = round_dn(psize_ / close_prices.len() as f64, qty_step);
    for &price in &close_prices[..close_prices.len() - 1] {
        let min_close_qty = calc_min_entry_qty(price, inverse, c_mult, qty_step, min_qty, min_cost);
        if psize_ < min_close_qty {
            break;
        }
        let close_qty = psize_.min(default_close_qty.max(min_close_qty));
        closes.push((close_qty, price, "short_nclose"));
        psize_ = round_(psize_ - close_qty, qty_step);
    }

    let last_price = if let Some(price) = close_prices.last() {
        *price
    } else {
        // This case should be handled by the is_empty check above, but for safety:
        return if closes.is_empty() {
            vec![(0.0, 0.0, "")]
        } else {
            closes
        };
    };
    let min_close_qty =
        calc_min_entry_qty(last_price, inverse, c_mult, qty_step, min_qty, min_cost);
    if psize_ >= min_close_qty {
        closes.push((psize_, last_price, "short_nclose"));
    } else if let Some(last_close) = closes.last_mut() {
        *last_close = (
            round_(last_close.0.abs() + psize_, qty_step),
            last_close.1,
            last_close.2,
        );
    }

    if closes.is_empty() {
        vec![(0.0, 0.0, "")]
    } else {
        closes
    }
}

/// Calculates a grid of close orders for a short position using the "backwards" distribution method.
///
/// In this method, a fixed quantity is placed at each price level, starting from the furthest
/// price and working backwards. This prioritizes filling the further-away orders.
///
/// # Returns
///
/// A `Vec` of tuples, where each tuple represents a close order: `(quantity, price, label)`.
/// Quantity is positive for close orders.
pub fn calc_close_grid_backwards_short(
    balance: f64, psize: f64, pprice: f64, highest_bid: f64, ema_band_lower: f64, utc_now_ms: f64,
    prev_au_fill_ts_close: f64, inverse: bool, qty_step: f64, price_step: f64, min_qty: f64,
    min_cost: f64, c_mult: f64, wallet_exposure_limit: f64, min_markup: f64, markup_range: f64,
    n_close_orders: f64, auto_unstuck_wallet_exposure_threshold: f64, auto_unstuck_ema_dist: f64,
    auto_unstuck_delay_minutes: f64, auto_unstuck_qty_pct: f64,
) -> Vec<(f64, f64, &'static str)> {
    let mut psize_ = round_dn(psize.abs(), qty_step);
    if psize_ == 0.0 {
        return vec![(0.0, 0.0, "")];
    }
    let full_psize = cost_to_qty(balance * wallet_exposure_limit, pprice, inverse, c_mult);

    let n_close_orders_f = n_close_orders
        .min(full_psize / calc_min_entry_qty(pprice, inverse, c_mult, qty_step, min_qty, min_cost))
        .max(1.0);
    let n_close_orders = n_close_orders_f.round() as i32;

    let raw_close_prices =
        generate_raw_close_prices(pprice, min_markup, markup_range, n_close_orders, SHORT);

    let mut close_prices_all = Vec::new();
    let mut close_prices = Vec::new();
    for p in raw_close_prices {
        let price = round_dn(p, price_step);
        if !close_prices_all.contains(&price) {
            close_prices_all.push(price);
            if price <= highest_bid {
                close_prices.push(price);
            }
        }
    }

    if close_prices.is_empty() {
        return vec![(psize_, highest_bid, "short_nclose")];
    }

    let mut closes = Vec::new();
    if auto_unstuck_wallet_exposure_threshold != 0.0 {
        let auto_unstuck_close = calc_auto_unstuck_close_short(
            balance,
            psize,
            pprice,
            highest_bid,
            ema_band_lower,
            utc_now_ms,
            prev_au_fill_ts_close,
            inverse,
            qty_step,
            price_step,
            min_qty,
            min_cost,
            c_mult,
            wallet_exposure_limit,
            auto_unstuck_wallet_exposure_threshold,
            auto_unstuck_ema_dist,
            auto_unstuck_delay_minutes,
            auto_unstuck_qty_pct,
            close_prices[0],
        );
        if auto_unstuck_close.0 != 0.0 {
            psize_ = round_(psize_ - auto_unstuck_close.0.abs(), qty_step);
            let min_entry_qty = calc_min_entry_qty(
                auto_unstuck_close.1,
                inverse,
                c_mult,
                qty_step,
                min_qty,
                min_cost,
            );
            if psize_ < min_entry_qty {
                return vec![(psize.abs(), auto_unstuck_close.1, "unstuck_close_short")];
            }
            closes.push(auto_unstuck_close);
        }
    }

    if close_prices.len() == 1 {
        let min_entry_qty = calc_min_entry_qty(
            close_prices[0],
            inverse,
            c_mult,
            qty_step,
            min_qty,
            min_cost,
        );
        if psize_ >= min_entry_qty {
            closes.push((psize_, close_prices[0], "short_nclose"));
        }
        return if closes.is_empty() {
            vec![(0.0, 0.0, "")]
        } else {
            closes
        };
    }

    let qty_per_close = (full_psize / close_prices_all.len() as f64).max(min_qty);
    let qty_per_close = round_up(qty_per_close, qty_step);

    for &price in close_prices.iter().rev() {
        let min_entry_qty = calc_min_entry_qty(price, inverse, c_mult, qty_step, min_qty, min_cost);
        let qty = psize_.min(qty_per_close.max(min_entry_qty));
        if qty < min_entry_qty {
            if let Some(last_close) = closes.last_mut() {
                *last_close = (
                    round_(last_close.0.abs() + psize_, qty_step),
                    last_close.1,
                    last_close.2,
                );
            } else {
                closes.push((psize_, price, "short_nclose"));
            }
            psize_ = 0.0;
            break;
        }
        closes.push((qty, price, "short_nclose"));
        psize_ = round_(psize_ - qty, qty_step);
        if psize_ <= 0.0 {
            break;
        }
    }

    if psize_ > 0.0 {
        if let Some(last_close) = closes.last_mut() {
            *last_close = (
                round_(last_close.0.abs() + psize_, qty_step),
                last_close.1,
                last_close.2,
            );
        }
    }

    closes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    if closes.is_empty() {
        vec![(0.0, 0.0, "")]
    } else {
        closes
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_() {
        assert_eq!(round_(1.2345, 0.01), 1.23);
        assert_eq!(round_(1.2355, 0.01), 1.24);
        assert_eq!(round_(1.23, 0.05), 1.25);
        assert_eq!(round_(1.22, 0.05), 1.20);
    }

    #[test]
    fn test_round_up() {
        assert_eq!(round_up(1.2345, 0.01), 1.24);
        assert_eq!(round_up(1.23, 0.01), 1.23);
        assert_eq!(round_up(1.23, 0.05), 1.25);
        assert_eq!(round_up(1.20, 0.05), 1.20);
    }

    #[test]
    fn test_round_dn() {
        assert_eq!(round_dn(1.2345, 0.01), 1.23);
        assert_eq!(round_dn(1.2399, 0.01), 1.23);
        assert_eq!(round_dn(1.23, 0.01), 1.23);
        assert_eq!(round_dn(1.24, 0.05), 1.20);
    }
}
#[test]
fn test_calc_diff() {
    let epsilon = 1e-9;
    assert!((calc_diff(1.0, 1.0) - 0.0).abs() < epsilon);
    assert!((calc_diff(1.1, 1.0) - 0.1).abs() < epsilon);
    assert!((calc_diff(0.9, 1.0) - 0.1).abs() < epsilon);
    assert_eq!(calc_diff(0.0, 0.0), 0.0);
    assert_eq!(calc_diff(1.0, 0.0), f64::INFINITY);
}

#[test]
fn test_cost_to_qty() {
    // Linear
    assert_eq!(cost_to_qty(100.0, 50.0, false, 1.0), 2.0);
    assert_eq!(cost_to_qty(100.0, 0.0, false, 1.0), 0.0);

    // Inverse
    assert_eq!(cost_to_qty(2.0, 50.0, true, 1.0), 100.0);
    assert_eq!(cost_to_qty(100.0, 50.0, true, 2.0), 2500.0);
}

#[test]
fn test_qty_to_cost() {
    // Linear
    assert_eq!(qty_to_cost(2.0, 50.0, false, 1.0), 100.0);
    assert_eq!(qty_to_cost(-2.0, 50.0, false, 1.0), 100.0);

    // Inverse
    assert_eq!(qty_to_cost(100.0, 50.0, true, 1.0), 2.0);
    assert_eq!(qty_to_cost(100.0, 0.0, true, 1.0), 0.0);
    assert_eq!(qty_to_cost(2500.0, 50.0, true, 2.0), 100.0);
}
#[test]
fn test_calc_pnl_long() {
    let epsilon = 1e-9;
    // Linear
    assert!((calc_pnl_long(100.0, 110.0, 1.0, false, 1.0) - 10.0).abs() < epsilon);
    // Inverse
    assert!((calc_pnl_long(100.0, 110.0, 11000.0, true, 1.0) - 10.0).abs() < epsilon);
}

#[test]
fn test_calc_pnl_short() {
    let epsilon = 1e-9;
    // Linear
    assert!((calc_pnl_short(100.0, 90.0, 1.0, false, 1.0) - 10.0).abs() < epsilon);
    // Inverse
    assert!((calc_pnl_short(100.0, 90.0, 9000.0, true, 1.0) - 10.0).abs() < epsilon);
}

#[test]
fn test_calc_new_psize_pprice() {
    let epsilon = 1e-9;
    let (psize, pprice) = calc_new_psize_pprice(1.0, 100.0, 1.0, 110.0, 0.01);
    assert!((psize - 2.0).abs() < epsilon);
    assert!((pprice - 105.0).abs() < epsilon);

    let (psize, pprice) = calc_new_psize_pprice(0.0, 0.0, 1.0, 110.0, 0.01);
    assert!((psize - 1.0).abs() < epsilon);
    assert!((pprice - 110.0).abs() < epsilon);

    let (psize, pprice) = calc_new_psize_pprice(1.0, 100.0, -1.0, 110.0, 0.01);
    assert!((psize - 0.0).abs() < epsilon);
    assert!((pprice - 0.0).abs() < epsilon);
}
