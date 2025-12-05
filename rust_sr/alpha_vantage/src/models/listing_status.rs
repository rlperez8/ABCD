use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListingStatus {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    pub assetType: String,
    pub ipoDate: String,
    pub delistingDate: String,
    pub status: String,
    pub average_volume: Option<f64>, 
}