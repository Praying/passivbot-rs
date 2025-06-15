use crate::types::BotConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use crate::exchange::SendSyncError;

#[derive(Deserialize, Debug, Clone)]
pub struct UserConfig {
    pub exchange: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub secret: String,
    #[serde(default)]
    pub passphrase: String,
    #[serde(default)]
    pub wallet_address: String,
    #[serde(default)]
    pub private_key: String,
    #[serde(default)]
    pub is_vault: bool,
}

pub fn load_api_keys() -> Result<HashMap<String, UserConfig>, SendSyncError> {
    let content = fs::read_to_string("api-keys.json").map_err(|e| Box::new(e) as SendSyncError)?;
    let api_keys: HashMap<String, UserConfig> =
        serde_json::from_str(&content).map_err(|e| Box::new(e) as SendSyncError)?;
    Ok(api_keys)
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<BotConfig, SendSyncError> {
    let content = fs::read_to_string(path).map_err(|e| Box::new(e) as SendSyncError)?;
    let config: BotConfig = serde_hjson::from_str(&content).map_err(|e| Box::new(e) as SendSyncError)?;
    Ok(config)
}