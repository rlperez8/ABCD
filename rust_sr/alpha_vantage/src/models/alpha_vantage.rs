use crate::models::candle::Candle;
use serde_json::Value;
use chrono::NaiveDate;
use std::error::Error;
use std::fmt;
use tokio::time::{sleep, Duration};

const MAX_RATE_LIMIT_RETRIES: usize = 3;
const RATE_LIMIT_DELAY_SECONDS: u64 = 20;

fn is_rate_limit_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    normalized.contains("rate limit")
        || normalized.contains("burst pattern detected")
        || normalized.contains("requests per minute")
        || normalized.contains("requests per second")
        || normalized.contains("premium subscription plan")
}

#[derive(Debug)]
pub enum AlphaVantageError {
    Api(String),
    RateLimited(String),
    UnexpectedResponse(String),
    Request(reqwest::Error),
    Json(reqwest::Error),
    CandleParse(serde_json::Error),
    DateParse(chrono::ParseError),
}

impl fmt::Display for AlphaVantageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api(message) => write!(f, "Alpha Vantage API error: {}", message),
            Self::RateLimited(message) => write!(f, "Alpha Vantage rate limit hit: {}", message),
            Self::UnexpectedResponse(message) => {
                write!(f, "Alpha Vantage returned an unexpected response: {}", message)
            }
            Self::Request(error) => write!(f, "Request failed: {}", error),
            Self::Json(error) => write!(f, "Failed to decode API response JSON: {}", error),
            Self::CandleParse(error) => write!(f, "Failed to parse candle payload: {}", error),
            Self::DateParse(error) => write!(f, "Failed to parse candle date: {}", error),
        }
    }
}

impl Error for AlphaVantageError {}

impl From<reqwest::Error> for AlphaVantageError {
    fn from(error: reqwest::Error) -> Self {
        Self::Request(error)
    }
}

pub struct AlphaVantage {
    pub api_key: String,
}

impl AlphaVantage {
    
    pub async fn load_single_symbol_candle_data(
        &self,
        output_size: &str,
        ticker: &str,
    ) -> Result<Vec<Candle>, AlphaVantageError> {
        let timeframe = "TIME_SERIES_DAILY";
        let url = format!(
            "https://www.alphavantage.co/query?function={}&symbol={}&outputsize={}&apikey={}",
            timeframe, ticker, output_size, self.api_key
        );

        for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
            let response = reqwest::get(&url)
                .await
                .map_err(AlphaVantageError::Request)?
                .error_for_status()
                .map_err(AlphaVantageError::Request)?;
            let data: Value = response.json().await.map_err(AlphaVantageError::Json)?;

            if let Some(note) = data.get("Note").and_then(Value::as_str) {
                if attempt < MAX_RATE_LIMIT_RETRIES {
                    eprintln!(
                        "Rate limit for {}. Waiting {} seconds before retry {}/{}.",
                        ticker,
                        RATE_LIMIT_DELAY_SECONDS,
                        attempt + 1,
                        MAX_RATE_LIMIT_RETRIES
                    );
                    sleep(Duration::from_secs(RATE_LIMIT_DELAY_SECONDS)).await;
                    continue;
                }

                return Err(AlphaVantageError::RateLimited(note.to_string()));
            }

            if let Some(message) = data.get("Error Message").and_then(Value::as_str) {
                return Err(AlphaVantageError::Api(message.to_string()));
            }

            if let Some(message) = data.get("Information").and_then(Value::as_str) {
                if is_rate_limit_message(message) {
                    if attempt < MAX_RATE_LIMIT_RETRIES {
                        eprintln!(
                            "Rate limit for {}. Waiting {} seconds before retry {}/{}.",
                            ticker,
                            RATE_LIMIT_DELAY_SECONDS,
                            attempt + 1,
                            MAX_RATE_LIMIT_RETRIES
                        );
                        sleep(Duration::from_secs(RATE_LIMIT_DELAY_SECONDS)).await;
                        continue;
                    }

                    return Err(AlphaVantageError::RateLimited(message.to_string()));
                }

                return Err(AlphaVantageError::UnexpectedResponse(message.to_string()));
            }

            let time_series = data
                .get("Time Series (Daily)")
                .and_then(Value::as_object)
                .ok_or_else(|| {
                    AlphaVantageError::UnexpectedResponse(format!(
                        "Ticker {} was missing Time Series (Daily)",
                        ticker
                    ))
                })?;

            let mut candles: Vec<Candle> = Vec::new();

            for (date_str, values) in time_series.iter() {
                let mut candle: Candle =
                    serde_json::from_value(values.clone()).map_err(AlphaVantageError::CandleParse)?;
                candle.date = Some(
                    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                        .map_err(AlphaVantageError::DateParse)?,
                );
                candle.symbol = Some(ticker.to_string());
                candles.push(candle);
            }

            candles.sort_by_key(|c| c.date);
            return Ok(candles);
        }

        Err(AlphaVantageError::UnexpectedResponse(format!(
            "Ticker {} exhausted retries without a usable response",
            ticker
        )))
    }
}
