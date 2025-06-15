use crate::types::{BotConfig, ExchangeConfig};
use anyhow::{anyhow, Result};
use bytes::Bytes;
use chrono::{NaiveDate, Utc};
use csv::ReaderBuilder;
use futures::future::join_all;
use ndarray::Array2;
use ndarray_npy::WriteNpyExt;
use std::fs::{self, File};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};
use zip::ZipArchive;

pub struct Downloader {
    pub config: BotConfig,
}

impl Downloader {
    pub fn new(config: BotConfig) -> Self {
        Downloader { config }
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting downloader...");

        let backtest_config = &self.config.backtest;

        for (exchange_name, exchange_config) in &backtest_config.exchanges {
            info!("Downloading data for exchange: {}", exchange_name);
            let symbols = match backtest_config.symbols.get(exchange_name) {
                Some(s) => s,
                None => {
                    warn!("No symbols configured for exchange: {}", exchange_name);
                    continue;
                }
            };

            match exchange_name.as_str() {
                "binance" => {
                    self.download_binance_data(
                        symbols,
                        exchange_config,
                        &backtest_config.start_date,
                        &backtest_config.end_date,
                    )
                    .await?
                }
                "bybit" => {
                    self.download_bybit_data(
                        symbols,
                        &backtest_config.start_date,
                        &backtest_config.end_date,
                    )
                    .await?
                }
                _ => warn!("Exchange '{}' is not supported by the downloader.", exchange_name),
            }
        }

        info!("Downloader finished.");
        Ok(())
    }

    async fn download_binance_data(
        &self,
        symbols: &Vec<String>,
        exchange_config: &ExchangeConfig,
        start_date_str: &str,
        end_date_str: &str,
    ) -> Result<()> {
        let start_date = NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d")?;
        let end_date = NaiveDate::parse_from_str(end_date_str, "%Y-%m-%d")?;
        let market_type = if exchange_config.spot { "spot" } else { "futures/um" };

        for symbol in symbols {
            info!("Downloading Binance data for {}", symbol);
            let dir_path = PathBuf::from(format!(
                "historical_data/ohlcvs_{}/{}/",
                if exchange_config.spot { "spot" } else { "futures" },
                symbol
            ));
            fs::create_dir_all(&dir_path)?;

            let (months, days) = self.get_date_ranges(start_date, end_date);

            // Download monthly data
            let mut tasks = vec![];
            for month in months {
                let month_path = dir_path.join(format!("{}.npy", month));
                if !month_path.exists() {
                    let url = format!(
                        "https://data.binance.vision/data/{}/monthly/klines/{}/1m/{}-1m-{}.zip",
                        market_type, symbol, symbol, month
                    );
                    tasks.push(self.download_and_process_zip(url, month_path.clone()));
                }
            }
            join_all(tasks).await;

            // Download daily data
            let mut tasks = vec![];
            for day in days {
                let day_path = dir_path.join(format!("{}.npy", day));
                let month_of_day = &day[0..7];
                let month_path = dir_path.join(format!("{}.npy", month_of_day));
                if !day_path.exists() && !month_path.exists() {
                    let url = format!(
                        "https://data.binance.vision/data/{}/daily/klines/{}/1m/{}-1m-{}.zip",
                        market_type, symbol, symbol, day
                    );
                    tasks.push(self.download_and_process_zip(url, day_path.clone()));
                }
            }
            join_all(tasks).await;

            self.cleanup_daily_files(&dir_path)?;
        }
        Ok(())
    }

    async fn download_and_process_zip(&self, url: String, npy_path: PathBuf) -> Result<()> {
        info!("Fetching {}", url);
        match reqwest::get(&url).await {
            Ok(response) => {
                if response.status().is_success() {
                    let zip_bytes = response.bytes().await?;
                    self.process_zip_data(zip_bytes, npy_path)?;
                } else {
                    warn!("Failed to download {}: Status {}", url, response.status());
                }
            }
            Err(e) => error!("Error downloading {}: {}", url, e),
        }
        // Rate limit
        sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    fn process_zip_data(&self, zip_bytes: Bytes, npy_path: PathBuf) -> Result<()> {
        let cursor = Cursor::new(zip_bytes);
        let mut archive = ZipArchive::new(cursor)?;

        if archive.len() == 0 {
            return Err(anyhow!("ZIP archive is empty"));
        }

        let mut file_in_zip = archive.by_index(0)?;
        let mut csv_data = String::new();
        file_in_zip.read_to_string(&mut csv_data)?;

        let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(csv_data.as_bytes());
        let mut records = Vec::new();
        for result in rdr.records() {
            let record = result?;
            // timestamp, open, high, low, close, volume
            let timestamp: f64 = record[0].parse()?;
            let open: f64 = record[1].parse()?;
            let high: f64 = record[2].parse()?;
            let low: f64 = record[3].parse()?;
            let close: f64 = record[4].parse()?;
            let volume: f64 = record[5].parse()?;
            records.push(vec![timestamp, open, high, low, close, volume]);
        }

        if records.is_empty() {
            return Err(anyhow!("No data in CSV"));
        }

        let array =
            Array2::from_shape_vec((records.len(), 6), records.into_iter().flatten().collect())?;

        let file = File::create(&npy_path)?;
        array.write_npy(file)?;

        info!("Saved data to {}", npy_path.display());
        Ok(())
    }

    fn get_date_ranges(&self, start: NaiveDate, end: NaiveDate) -> (Vec<String>, Vec<String>) {
        let mut months = Vec::new();
        let mut days = Vec::new();
        let mut current = start;

        while current <= end {
            let month_str = current.format("%Y-%m").to_string();
            if !months.contains(&month_str) {
                months.push(month_str);
            }
            days.push(current.format("%Y-%m-%d").to_string());
            current = current.succ_opt().unwrap();
        }

        // Do not try to download monthly data for the current month
        let current_month = Utc::now().format("%Y-%m").to_string();
        months.retain(|m| m != &current_month);

        (months, days)
    }

    fn cleanup_daily_files(&self, dir_path: &Path) -> Result<()> {
        let entries = fs::read_dir(dir_path)?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect::<Vec<_>>();

        for path in &entries {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                // stem is YYYY-MM-DD
                if stem.len() == 10 {
                    let month_path = dir_path.join(format!("{}.npy", &stem[0..7]));
                    if month_path.exists() {
                        info!("Deleting daily file {} as monthly file exists.", path.display());
                        fs::remove_file(path)?;
                    }
                }
            }
        }
        Ok(())
    }

    async fn download_bybit_data(
        &self,
        symbols: &Vec<String>,
        _start_date_str: &str,
        _end_date_str: &str,
    ) -> Result<()> {
        let data_dir = Path::new("data");
        if !data_dir.exists() {
            fs::create_dir(data_dir)?;
        }

        for symbol in symbols {
            info!("Downloading Bybit data for {}", symbol);
            // The existing bybit logic is very basic and needs a full rewrite
            // to match python version's capabilities (fetching from public.bybit.com,
            // processing trades into ohlcv, etc).
            // For now, we just log a warning.
            warn!("Bybit downloader is not fully implemented yet. Skipping {}.", symbol);
        }
        Ok(())
    }
}