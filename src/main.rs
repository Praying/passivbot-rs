#![allow(dead_code)]
#![allow(unused_variables)]

mod types;
mod config;
mod grid;
mod constants;
mod exchange;
mod bot;
mod manager;
mod backtest;
mod forager;
mod data;
mod optimizer;
mod downloader;
pub mod analysis;
pub mod profit_transfer;

use crate::config::{load_api_keys, UserConfig};
use crate::exchange::{Exchange, SendSyncError};
use crate::types::LiveConfig;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Runs the live trading bot
    Live {
        #[clap(long)]
        user: String,
    },
    /// Runs a backtest
    Backtest,
    /// Runs the optimizer
    Optimize,
    /// Downloads historical data
    Download,
    /// Transfers profits from futures to spot
    ProfitTransfer(profit_transfer::ProfitTransferArgs),
}

fn init_exchange(
    live_config: &LiveConfig,
    user_config: &UserConfig,
) -> Result<Box<dyn Exchange>, SendSyncError> {
    match user_config.exchange.as_str() {
        "bybit" => Ok(Box::new(exchange::bybit::Bybit::new(
            live_config,
            user_config,
        ))),
        "binance" => Ok(Box::new(exchange::binance::Binance::new(
            live_config,
            user_config,
        ))),
        "bitget" => Ok(Box::new(exchange::bitget::Bitget::new(
            live_config,
            user_config,
        ))),
        "gateio" => Ok(Box::new(exchange::gateio::Gateio::new(
            live_config,
            user_config,
        ))),
        "hyperliquid" => Ok(Box::new(exchange::hyperliquid::Hyperliquid::new(
            live_config,
            user_config,
        ))),
        "okx" => Ok(Box::new(exchange::okx::Okx::new(live_config, user_config))),
        _ => Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Unsupported exchange: {}", user_config.exchange),
        ))),
    }
}

#[tokio::main]
async fn main() -> Result<(), SendSyncError> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let config = match config::load_config("config.hjson") {
        Ok(config) => config,
        Err(e) => return Err(e),
    };
    
    let api_keys = load_api_keys()?;

    match &cli.command {
        Commands::Live { user } => {
            let user_config = api_keys.get(user).ok_or("User not found in api-keys.json")?;
            let exchange = init_exchange(&config.live, user_config)?;
            let mut bot = bot::Passivbot::new(config, exchange);
            bot.start().await?;
        }
        Commands::Backtest => {
            let mut backtester = backtest::Backtester::new(config);
            backtester.start().await?;
        }
        Commands::Optimize => {
            let mut optimizer = optimizer::Optimizer::new(config);
            optimizer.start().await?;
        }
        Commands::Download => {
            let downloader = downloader::Downloader::new(config);
            downloader.start().await?;
        }
        Commands::ProfitTransfer(args) => {
            let user_config = api_keys
                .get(&args.user)
                .ok_or("User not found in api-keys.json")?;
            let exchange = init_exchange(&config.live, user_config)?;
            let mut transferer =
                profit_transfer::ProfitTransferer::new(exchange, args.clone());
            transferer.start().await?;
        }
    }

    Ok(())
}
