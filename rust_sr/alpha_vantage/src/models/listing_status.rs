use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ListingStatus {
    pub symbol: String,
    pub name: String,
    pub exchange: String,
    #[serde(rename = "assetType")]
    pub asset_type: String,
    #[serde(rename = "ipoDate")]
    pub ipo_date: String,
    #[serde(rename = "delistingDate")]
    pub delisting_date: String,
    pub status: String,
    #[serde(default)]
    pub average_volume_30d: Option<f64>, 
}
