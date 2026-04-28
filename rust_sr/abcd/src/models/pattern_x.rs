use crate::models::pivot::Pivot;
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PatternX {
    pub x: Pivot,
    #[serde(skip_serializing)]
    pub x_index: usize,
}
