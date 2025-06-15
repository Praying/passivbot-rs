use std::collections::HashMap;
use std::fmt;
use crate::types::{OrderBook, Position};

#[derive(Debug)]
pub struct ExchangeParams {
    pub qty_step: f64,
    pub price_step: f64,
    pub min_qty: f64,
    pub min_cost: f64,
    pub c_mult: f64,
}

impl Default for ExchangeParams {
    fn default() -> Self {
        ExchangeParams {
            qty_step: 0.00001,
            price_step: 0.00001,
            min_qty: 0.00001,
            min_cost: 1.0,
            c_mult: 1.0,
        }
    }
}

#[derive(Clone)]
pub struct BacktestParams {
    pub starting_balance: f64,
    pub maker_fee: f64,
    pub symbols: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Positions {
    pub long: HashMap<usize, Position>,
    pub short: HashMap<usize, Position>,
}

#[derive(Debug, Default, Clone)]
pub struct EMABands {
    pub upper: f64,
    pub lower: f64,
}


#[derive(Debug, Default, Clone)]
pub struct StateParams {
    pub balance: f64,
    pub order_book: OrderBook,
    pub ema_bands: EMABands,
}

#[derive(Clone, Default, Debug)]
pub struct BotParamsPair {
    pub long: BotParams,
    pub short: BotParams,
}

#[derive(Clone, Default, Debug)]
pub struct BotParams {
    pub close_grid_markup_range: f64,
    pub close_grid_min_markup: f64,
    pub close_grid_qty_pct: f64,
    pub close_trailing_retracement_pct: f64,
    pub close_trailing_grid_ratio: f64,
    pub close_trailing_qty_pct: f64,
    pub close_trailing_threshold_pct: f64,
    pub entry_grid_double_down_factor: f64,
    pub entry_grid_spacing_weight: f64,
    pub entry_grid_spacing_pct: f64,
    pub entry_initial_ema_dist: f64,
    pub entry_initial_qty_pct: f64,
    pub entry_trailing_retracement_pct: f64,
    pub entry_trailing_grid_ratio: f64,
    pub entry_trailing_threshold_pct: f64,
    pub filter_rolling_window: usize,
    pub filter_relative_volume_clip_pct: f64,
    pub ema_span_0: f64,
    pub ema_span_1: f64,
    pub n_positions: usize,
    pub total_wallet_exposure_limit: f64,
    pub wallet_exposure_limit: f64, // is total_wallet_exposure_limit / n_positions
    pub unstuck_close_pct: f64,
    pub unstuck_ema_dist: f64,
    pub unstuck_loss_allowance_pct: f64,
    pub unstuck_threshold: f64,
}

#[derive(Debug)]
pub struct TrailingPriceBundle {
    pub min_since_open: f64,
    pub max_since_min: f64,
    pub max_since_open: f64,
    pub min_since_max: f64,
}
impl Default for TrailingPriceBundle {
    fn default() -> Self {
        TrailingPriceBundle {
            min_since_open: f64::MAX,
            max_since_min: 0.0,
            max_since_open: 0.0,
            min_since_max: f64::MAX,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OrderType {
    EntryInitialNormalLong,
    EntryInitialPartialLong,
    EntryTrailingNormalLong,
    EntryTrailingCroppedLong,
    EntryGridNormalLong,
    EntryGridCroppedLong,
    EntryGridInflatedLong,

    CloseGridLong,
    CloseTrailingLong,
    CloseUnstuckLong,

    EntryInitialNormalShort,
    EntryInitialPartialShort,
    EntryTrailingNormalShort,
    EntryTrailingCroppedShort,
    EntryGridNormalShort,
    EntryGridCroppedShort,
    EntryGridInflatedShort,

    CloseGridShort,
    CloseTrailingShort,
    CloseUnstuckShort,

    Empty,
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OrderType::EntryInitialNormalLong => write!(f, "entry_initial_normal_long"),
            OrderType::EntryInitialPartialLong => write!(f, "entry_initial_partial_long"),
            OrderType::EntryTrailingNormalLong => write!(f, "entry_trailing_normal_long"),
            OrderType::EntryTrailingCroppedLong => write!(f, "entry_trailing_cropped_long"),
            OrderType::EntryGridNormalLong => write!(f, "entry_grid_normal_long"),
            OrderType::EntryGridCroppedLong => write!(f, "entry_grid_cropped_long"),
            OrderType::EntryGridInflatedLong => write!(f, "entry_grid_inflated_long"),
            OrderType::CloseGridLong => write!(f, "close_grid_long"),
            OrderType::CloseTrailingLong => write!(f, "close_trailing_long"),
            OrderType::CloseUnstuckLong => write!(f, "close_unstuck_long"),
            OrderType::EntryInitialNormalShort => write!(f, "entry_initial_normal_short"),
            OrderType::EntryInitialPartialShort => write!(f, "entry_initial_partial_short"),
            OrderType::EntryTrailingNormalShort => write!(f, "entry_trailing_normal_short"),
            OrderType::EntryTrailingCroppedShort => write!(f, "entry_trailing_cropped_short"),
            OrderType::EntryGridNormalShort => write!(f, "entry_grid_normal_short"),
            OrderType::EntryGridCroppedShort => write!(f, "entry_grid_cropped_short"),
            OrderType::EntryGridInflatedShort => write!(f, "entry_grid_inflated_short"),
            OrderType::CloseGridShort => write!(f, "close_grid_short"),
            OrderType::CloseTrailingShort => write!(f, "close_trailing_short"),
            OrderType::CloseUnstuckShort => write!(f, "close_unstuck_short"),
            OrderType::Empty => write!(f, "empty"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub index: usize,
    pub symbol: String,
    pub pnl: f64,
    pub fee_paid: f64,
    pub balance: f64,
    pub fill_qty: f64,
    pub fill_price: f64,
    pub position_size: f64,
    pub position_price: f64,
    pub order_type: OrderType,
}

#[derive(Debug)]
pub struct Analysis {
    pub adg: f64,
    pub mdg: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub omega_ratio: f64,
    pub expected_shortfall_1pct: f64,
    pub calmar_ratio: f64,
    pub sterling_ratio: f64,
    pub drawdown_worst: f64,
    pub drawdown_worst_mean_1pct: f64,
    pub equity_balance_diff_mean: f64,
    pub equity_balance_diff_max: f64,
    pub loss_profit_ratio: f64,
}

impl Default for Analysis {
    fn default() -> Self {
        Analysis {
            adg: 0.0,
            mdg: 0.0,
            sharpe_ratio: 0.0,
            sortino_ratio: 0.0,
            omega_ratio: 0.0,
            expected_shortfall_1pct: 0.0,
            calmar_ratio: 0.0,
            sterling_ratio: 0.0,
            drawdown_worst: 1.0,
            drawdown_worst_mean_1pct: 1.0,
            equity_balance_diff_mean: 1.0,
            equity_balance_diff_max: 1.0,
            loss_profit_ratio: 1.0,
        }
    }
}