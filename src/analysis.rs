use crate::types::Analysis;
use statrs::statistics::Statistics;

pub fn calculate_metrics(equity_curve: &[f64]) -> Analysis {
    let mut analysis = Analysis::default();
    if equity_curve.len() < 2 {
        return analysis;
    }

    let returns = calculate_returns(equity_curve);

    analysis.drawdown_worst = calculate_max_drawdown(equity_curve);
    analysis.sharpe_ratio = calculate_sharpe_ratio(&returns);
    analysis.sortino_ratio = calculate_sortino_ratio(&returns);
    analysis.calmar_ratio = calculate_calmar_ratio(equity_curve, analysis.drawdown_worst);
    // TODO: Calculate other metrics

    analysis
}

/// Calculates the periodic returns from an equity curve.
fn calculate_returns(equity_curve: &[f64]) -> Vec<f64> {
    equity_curve
        .windows(2)
        .map(|w| (w[1] - w[0]) / w[0])
        .collect()
}

/// Calculates the Sharpe ratio from a slice of returns.
/// Assumes a risk-free rate of 0.
fn calculate_sharpe_ratio(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let mean = returns.mean();
    let std_dev = returns.std_dev();

    if std_dev == 0.0 {
        0.0
    } else {
        // Annualize the Sharpe Ratio, assuming daily data (252 trading days)
        mean / std_dev * (252.0_f64).sqrt()
    }
}

/// Calculates the Sortino ratio from a slice of returns.
fn calculate_sortino_ratio(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }
    let mean = returns.mean();
    
    // Calculate downside deviation
    let downside_returns: Vec<f64> = returns.iter().filter(|&&r| r < 0.0).map(|&r| r * r).collect();
    if downside_returns.is_empty() {
        return 0.0; // Or infinity, depending on convention
    }
    let len = downside_returns.len() as f64;
    if len < 2.0 {
        return 0.0;
    }
    let mean = downside_returns.mean();
    let downside_deviation = (mean * len / (len - 1.0)).sqrt();

    if downside_deviation == 0.0 {
        0.0
    } else {
        // Annualize the Sortino Ratio
        mean / downside_deviation * (252.0_f64).sqrt()
    }
}

/// Calculates the Calmar ratio.
fn calculate_calmar_ratio(equity_curve: &[f64], max_drawdown: f64) -> f64 {
    if max_drawdown == 0.0 || equity_curve.is_empty() {
        return 0.0;
    }

    if let (Some(last), Some(first)) = (equity_curve.last(), equity_curve.first()) {
        if *first == 0.0 {
            return 0.0; // Avoid division by zero
        }
        let total_return = (last - first) / first;
        let n_days = equity_curve.len() as f64;
        let annualized_return = (1.0 + total_return).powf(365.0 / n_days) - 1.0;
        annualized_return / max_drawdown
    } else {
        0.0
    }
}
/// Calculates the maximum drawdown from an equity curve.
/// The drawdown is the percentage loss from a peak to a subsequent trough.
fn calculate_max_drawdown(equity_curve: &[f64]) -> f64 {
    let mut max_drawdown = 0.0;
    let mut peak = equity_curve[0];

    for &equity in equity_curve.iter() {
        if equity > peak {
            peak = equity;
        }
        let drawdown = (peak - equity) / peak;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }
    max_drawdown
}