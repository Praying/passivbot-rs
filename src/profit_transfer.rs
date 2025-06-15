use crate::exchange::{Exchange, SendSyncError};
use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct ProfitTransferArgs {
    /// User/account name defined in api-keys.json
    #[clap(long)]
    pub user: String,

    /// Percentage to transfer, e.g., 0.5 for 50%
    #[clap(short, long, default_value_t = 0.5)]
    pub percentage: f64,

    /// Quote asset to transfer, e.g., USDT
    #[clap(short, long, default_value = "USDT")]
    pub quote: String,
}

pub struct ProfitTransferer {
    exchange: Box<dyn Exchange>,
    args: ProfitTransferArgs,
}

impl ProfitTransferer {
    pub fn new(exchange: Box<dyn Exchange>, args: ProfitTransferArgs) -> Self {
        Self { exchange, args }
    }

    pub async fn start(&mut self) -> Result<(), SendSyncError> {
        println!("Starting profit transfer for user: {}", self.args.user);
        // TODO: Implement the logic from the python script here
        Ok(())
    }
}
