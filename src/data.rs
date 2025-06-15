use crate::types::{BotConfig, LiveConfig};
use ndarray::Array2;
use tracing::info;
use csv;
use crate::exchange::SendSyncError;
use chrono::{NaiveDateTime, Utc};

pub async fn prepare_hlcvs(
    _config: &BotConfig, _exchange_config: &LiveConfig, symbol: &str, start_date: Option<&str>,
    end_date: Option<&str>,
) -> Result<Array2<f64>, SendSyncError> {
    info!("Preparing HLCV data for {} from local file...", symbol);

    let start_ts = start_date
        .and_then(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|dt| dt.and_local_timezone(Utc).unwrap().timestamp_millis() as u64);

    let end_ts = end_date
        .and_then(|s| NaiveDateTime::parse_from_str(s, "%Y-%m-%d").ok())
        .map(|dt| dt.and_local_timezone(Utc).unwrap().timestamp_millis() as u64);

    let file_path = format!("data/{}_1m.csv", symbol);
    let mut rdr = csv::Reader::from_path(file_path).map_err(|e| Box::new(e) as SendSyncError)?;

    let mut hlcvs = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| Box::new(e) as SendSyncError)?;
        let timestamp: u64 = record[0]
            .parse()
            .map_err(|e| Box::new(e) as SendSyncError)?;

        if let Some(start) = start_ts {
            if timestamp < start {
                continue;
            }
        }
        if let Some(end) = end_ts {
            if timestamp > end {
                continue;
            }
        }

        hlcvs.push([
            record[2]
                .parse()
                .map_err(|e| Box::new(e) as SendSyncError)?, // high
            record[3]
                .parse()
                .map_err(|e| Box::new(e) as SendSyncError)?, // low
            record[4]
                .parse()
                .map_err(|e| Box::new(e) as SendSyncError)?, // close
            record[5]
                .parse()
                .map_err(|e| Box::new(e) as SendSyncError)?, // volume
            record[4]
                .parse()
                .map_err(|e| Box::new(e) as SendSyncError)?, // close (again, for the 5th column)
        ]);
    }

    if hlcvs.is_empty() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No data found for the specified date range",
        )));
    }

    let hlcvs = Array2::from_shape_vec((hlcvs.len(), 5), hlcvs.into_iter().flatten().collect())
        .map_err(|e| Box::new(e) as SendSyncError)?;

    Ok(hlcvs)
}
