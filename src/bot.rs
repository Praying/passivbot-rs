use crate::types::BotConfig;
use crate::exchange::{Exchange, SendSyncError};
use crate::manager::Manager;
use crate::forager::Forager;
use std::collections::HashMap;
use tracing::info;
use tokio::task;

pub struct Passivbot {
    pub config: BotConfig,
    pub exchange: Box<dyn Exchange>,
}

impl Passivbot {
    pub fn new(config: BotConfig, exchange: Box<dyn Exchange>) -> Self {
        Passivbot {
            config,
            exchange,
        }
    }

    pub async fn start(&mut self) -> Result<(), SendSyncError> {
        info!("Starting bot...");
        self.run().await?;
        Ok(())
    }

    pub async fn run(&mut self) -> Result<(), SendSyncError> {
        info!("Bot is running...");
        
        let manager = Manager::new("".into(), self.config.clone(), self.exchange.clone_box());
        let forager = Forager::new(manager.clone()).await;

        let mut handles = HashMap::new();

        loop {
            let symbols_to_trade = forager.run().await;

            // Start managers for new symbols
            for symbol in &symbols_to_trade {
                if !handles.contains_key(symbol) {
                    let mut manager = Manager::new(symbol.clone(), self.config.clone(), self.exchange.clone_box());
                    let handle = task::spawn(async move {
                        manager.run().await;
                    });
                    handles.insert(symbol.clone(), handle);
                }
            }

            // Stop managers for symbols that are no longer in the list
            let symbols_to_stop: Vec<String> = handles
                .keys()
                .filter(|s| !symbols_to_trade.contains(s))
                .cloned()
                .collect();

            for symbol in symbols_to_stop {
                if let Some(handle) = handles.remove(&symbol) {
                    handle.abort();
                }
            }
            
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }
}