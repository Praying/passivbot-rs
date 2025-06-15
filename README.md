# passivbot-rs

[![Crates.io](https://img.shields.io/crates/v/passivbot-rs.svg)](https://crates.io/crates/passivbot-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![CI](https://github.com/Praying/passivbot-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/Praying/passivbot-rs/actions/workflows/rust.yml)
A high-performance cryptocurrency quantitative trading bot written in Rust.

## Core Features

-   **Live Trading**: Connect to real exchange accounts for automated trading.
-   **Strategy Backtesting**: Test the performance of trading strategies on historical data.
-   **Parameter Optimization**: Find the optimal parameters for your strategies.
-   **Data Downloading**: Download historical K-line data from exchanges.
-   **Profit Transfer**: Automatically transfer profits from futures to spot accounts.

## Supported Exchanges

-   Bybit
-   Binance
-   Bitget
-   Gate.io
-   Hyperliquid
-   OKX

## Installation Guide

### Prerequisites

-   [Rust](https://www.rust-lang.org/tools/install)

### Build Steps

1.  Clone the repository:
    ```bash
    git clone https://github.com/your-username/passivbot-rs.git
    cd passivbot-rs
    ```
2.  Build the project:
    ```bash
    cargo build --release
    ```
    The executable will be located at `target/release/passivbot-rs`.

## Configuration

### `config.hjson`

The main configuration file for the bot. Here you can configure parameters for live trading, backtesting, optimization, and the trading strategy itself.

### `api-keys.json`

To use the live trading and profit transfer features, you need to create an `api-keys.json` file in the root directory of the project. This file should contain the API key and secret for the exchange you want to use.

**`api-keys.json` example:**

```json
{
  "test_user": {
    "exchange": "bybit",
    "api_key": "YOUR_API_KEY",
    "api_secret": "YOUR_API_SECRET"
  }
}
```

## Usage

### Live Trading

```bash
./target/release/passivbot-rs live --user test_user
```

### Strategy Backtesting

```bash
./target/release/passivbot-rs backtest
```

### Parameter Optimization

```bash
./target/release/passivbot-rs optimize
```

### Data Downloading

```bash
./target/release/passivbot-rs download
```

### Profit Transfer

```bash
./target/release/passivbot-rs profit-transfer --user test_user --amount 100 --asset USDT
```

## Risk Disclaimer

Trading cryptocurrencies involves significant risk. This bot is provided "as is", and the author is not responsible for any financial losses you may incur. Always do your own research and use this bot at your own risk.