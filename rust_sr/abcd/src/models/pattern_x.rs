use serde::Serialize;
use crate::models::pivot::Pivot;
#[derive(Debug, Clone, Serialize)]
pub struct PatternX {
    pub x: Pivot,
}