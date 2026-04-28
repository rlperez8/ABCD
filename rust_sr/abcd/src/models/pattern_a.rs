use crate::models::pivot::Pivot;
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PatternXA {
    pub x: Pivot,
    pub a: Pivot,
    #[serde(skip_serializing)]
    pub x_index: usize,
    #[serde(skip_serializing)]
    pub a_index: usize,
}
