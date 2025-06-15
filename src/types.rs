use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

#[derive(Deserialize, Debug, Clone)]
pub struct BotConfig {
    pub live: LiveConfig,
    pub bot: SideConfigs,
    pub optimizer: OptimizerConfig,
    pub backtest: BacktestConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LiveConfig {
    pub exchange: String,
    #[serde(default)]
    pub approved_coins: Vec<String>,
    #[serde(default)]
    pub auto_gs: bool,
    #[serde(default)]
    pub coin_flags: HashMap<String, String>,
    #[serde(default)]
    pub empty_means_all_approved: bool,
    pub execution_delay_seconds: f64,
    #[serde(default)]
    pub filter_by_min_effective_cost: bool,
    #[serde(default)]
    pub forced_mode_long: String,
    #[serde(default)]
    pub forced_mode_short: String,
    #[serde(default)]
    pub ignored_coins: Vec<String>,
    pub leverage: f64,
    #[serde(default)]
    pub max_n_cancellations_per_batch: i32,
    #[serde(default)]
    pub max_n_creations_per_batch: i32,
    #[serde(default)]
    pub max_n_restarts_per_day: i32,
    pub minimum_coin_age_days: f64,
    #[serde(rename = "min_vol_24h")]
    pub min_vol_24h: f64,
    pub ohlcvs_1m_update_after_minutes: f64,
    pub ohlcvs_1m_rolling_window_days: f64,
    #[serde(default)]
    pub pnls_max_lookback_days: f64,
    #[serde(default)]
    pub price_distance_threshold: f64,
    #[serde(default)]
    pub time_in_force: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SideConfigs {
    pub long: BotSideConfig,
    pub short: BotSideConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OptimizeInRange {
    pub start: f64,
    pub end: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OptimizerConfig {
    pub n_generations: i32,
    pub population_size: i32,
    pub n_cpus: i32,
    pub backtest_n_days: i32,
    #[serde(default)]
    pub long: HashMap<String, OptimizeInRange>,
    #[serde(default)]
    pub short: HashMap<String, OptimizeInRange>,
    #[serde(default)]
    pub compress_results_file: bool,
    #[serde(default)]
    pub crossover_probability: f64,
    #[serde(default)]
    pub limits: HashMap<String, f64>,
    #[serde(default)]
    pub mutation_probability: f64,
    #[serde(default)]
    pub scoring: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExchangeConfig {
    #[serde(default)]
    pub spot: bool,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BacktestConfig {
    pub symbols: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub base_dir: String,
    #[serde(default)]
    pub compress_cache: bool,
    #[serde(default)]
    pub end_date: String,
    #[serde(default)]
    pub exchanges: HashMap<String, ExchangeConfig>,
    #[serde(default)]
    pub start_date: String,
    #[serde(default)]
    pub starting_balance: f64,
}

fn default_n_close_orders() -> f64 {
    5.0
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct BotSideConfig {
    pub total_wallet_exposure_limit: f64,
    pub n_positions: f64,
    pub unstuck_loss_allowance_pct: f64,
    pub unstuck_close_pct: f64,
    pub unstuck_ema_dist: f64,
    pub unstuck_threshold: f64,
    pub filter_rolling_window: f64,
    pub filter_relative_volume_clip_pct: f64,
    pub ema_span_0: f64,
    pub ema_span_1: f64,
    pub entry_initial_qty_pct: f64,
    pub entry_initial_ema_dist: f64,
    pub entry_grid_spacing_pct: f64,
    pub entry_grid_spacing_weight: f64,
    pub entry_grid_double_down_factor: f64,
    pub entry_trailing_threshold_pct: f64,
    pub entry_trailing_retracement_pct: f64,
    pub entry_trailing_grid_ratio: f64,
    pub close_grid_min_markup: f64,
    pub close_grid_markup_range: f64,
    pub close_grid_qty_pct: f64,
    #[serde(default = "default_n_close_orders")]
    pub n_close_orders: f64,
    pub close_trailing_threshold_pct: f64,
    pub close_trailing_retracement_pct: f64,
    pub close_trailing_qty_pct: f64,
    pub close_trailing_grid_ratio: f64,
    #[serde(default)]
    pub backwards_tp: bool,
}

#[derive(Deserialize, Debug, Default, Clone, Copy)]
pub struct Position {
    pub size: f64,
    pub price: f64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub position_side: String,
    pub qty: f64,
    pub price: f64,
    pub reduce_only: bool,
    pub custom_id: String,
    #[serde(default)]
    pub time_in_force: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Market {
    pub symbol: String,
    pub active: bool,
    pub swap: bool,
    pub linear: bool,
    #[serde(rename = "createdTime")]
    pub created_at: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Ticker {
    pub symbol: String,
    pub bid: f64,
    pub ask: f64,
    pub last: f64,
    #[serde(rename = "volume24h")]
    pub quote_volume: f64,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct OrderBook {
    pub bids: Vec<[f64; 2]>,
    pub asks: Vec<[f64; 2]>,
}

impl OrderBook {
    pub fn best_ask(&self) -> f64 {
        self.asks.get(0).map_or(f64::MAX, |a| a[0])
    }

    pub fn best_bid(&self) -> f64 {
        self.bids.get(0).map_or(0.0, |b| b[0])
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GridOrder {
    pub qty: f64,
    pub price: f64,
    pub order_type: OrderType,
}

#[derive(Debug, Clone)]
pub struct ExchangeParams {
    pub qty_step: f64,
    pub price_step: f64,
    pub min_qty: f64,
    pub min_cost: f64,
    pub c_mult: f64,
    pub inverse: bool,
}

impl Default for ExchangeParams {
    fn default() -> Self {
        ExchangeParams {
            qty_step: 0.00001,
            price_step: 0.00001,
            min_qty: 0.00001,
            min_cost: 1.0,
            c_mult: 1.0,
            inverse: false,
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

#[derive(Debug, Clone)]
pub struct StateParams {
    pub balance: f64,
    pub order_book: OrderBook,
    pub ema_bands: EMABands,
}

impl Default for StateParams {
    fn default() -> Self {
        StateParams {
            balance: 0.0,
            order_book: OrderBook::default(),
            ema_bands: EMABands::default(),
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct BotParamsPair {
    pub long: BotSideConfig,
    pub short: BotSideConfig,
}

#[derive(Debug, Clone)]
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
    EntryUnstuckLong,

    CloseGridLong,
    CloseTrailingLong,
    CloseNormalLong,
    CloseUnstuckLong,

    EntryInitialNormalShort,
    EntryInitialPartialShort,
    EntryTrailingNormalShort,
    EntryTrailingCroppedShort,
    EntryGridNormalShort,
    EntryGridCroppedShort,
    EntryGridInflatedShort,
    EntryUnstuckShort,

    CloseGridShort,
    CloseTrailingShort,
    CloseNormalShort,
    CloseUnstuckShort,

    Empty,
}

impl OrderType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "entry_initial_normal_long" => Some(OrderType::EntryInitialNormalLong),
            "entry_initial_partial_long" => Some(OrderType::EntryInitialPartialLong),
            "entry_trailing_normal_long" => Some(OrderType::EntryTrailingNormalLong),
            "entry_trailing_cropped_long" => Some(OrderType::EntryTrailingCroppedLong),
            "entry_grid_normal_long" => Some(OrderType::EntryGridNormalLong),
            "entry_grid_cropped_long" => Some(OrderType::EntryGridCroppedLong),
            "entry_grid_inflated_long" => Some(OrderType::EntryGridInflatedLong),
            "entry_unstuck_long" => Some(OrderType::EntryUnstuckLong),
            "close_grid_long" => Some(OrderType::CloseGridLong),
            "close_trailing_long" => Some(OrderType::CloseTrailingLong),
            "unstuck_close_long" => Some(OrderType::CloseUnstuckLong),
            "long_nclose" => Some(OrderType::CloseNormalLong),

            "entry_initial_normal_short" => Some(OrderType::EntryInitialNormalShort),
            "entry_initial_partial_short" => Some(OrderType::EntryInitialPartialShort),
            "entry_trailing_normal_short" => Some(OrderType::EntryTrailingNormalShort),
            "entry_trailing_cropped_short" => Some(OrderType::EntryTrailingCroppedShort),
            "entry_grid_normal_short" => Some(OrderType::EntryGridNormalShort),
            "entry_grid_cropped_short" => Some(OrderType::EntryGridCroppedShort),
            "entry_grid_inflated_short" => Some(OrderType::EntryGridInflatedShort),
            "entry_unstuck_short" => Some(OrderType::EntryUnstuckShort),
            "close_grid_short" => Some(OrderType::CloseGridShort),
            "close_trailing_short" => Some(OrderType::CloseTrailingShort),
            "unstuck_close_short" => Some(OrderType::CloseUnstuckShort),
            "short_nclose" => Some(OrderType::CloseNormalShort),

            _ => None,
        }
    }
}

impl Default for OrderType {
    fn default() -> Self {
        OrderType::Empty
    }
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
            OrderType::EntryUnstuckLong => write!(f, "entry_unstuck_long"),
            OrderType::CloseGridLong => write!(f, "close_grid_long"),
            OrderType::CloseTrailingLong => write!(f, "close_trailing_long"),
            OrderType::CloseNormalLong => write!(f, "long_nclose"),
            OrderType::CloseUnstuckLong => write!(f, "unstuck_close_long"),
            OrderType::EntryInitialNormalShort => write!(f, "entry_initial_normal_short"),
            OrderType::EntryInitialPartialShort => write!(f, "entry_initial_partial_short"),
            OrderType::EntryTrailingNormalShort => write!(f, "entry_trailing_normal_short"),
            OrderType::EntryTrailingCroppedShort => write!(f, "entry_trailing_cropped_short"),
            OrderType::EntryGridNormalShort => write!(f, "entry_grid_normal_short"),
            OrderType::EntryGridCroppedShort => write!(f, "entry_grid_cropped_short"),
            OrderType::EntryGridInflatedShort => write!(f, "entry_grid_inflated_short"),
            OrderType::EntryUnstuckShort => write!(f, "entry_unstuck_short"),
            OrderType::CloseGridShort => write!(f, "close_grid_short"),
            OrderType::CloseTrailingShort => write!(f, "close_trailing_short"),
            OrderType::CloseNormalShort => write!(f, "short_nclose"),
            OrderType::CloseUnstuckShort => write!(f, "unstuck_close_short"),
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
#[derive(Debug, Clone)]
pub struct Individual {
    pub config: BotConfig,
    pub fitness: f64,
}