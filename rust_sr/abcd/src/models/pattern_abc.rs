use crate::models::pivot::Pivot;
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PatternXABC {
    pub x: Pivot,
    pub a: Pivot,
    pub b: Pivot,
    pub c: Pivot,
    #[serde(skip_serializing)]
    pub x_index: usize,
    #[serde(skip_serializing)]
    pub a_index: usize,
    #[serde(skip_serializing)]
    pub b_index: usize,
    #[serde(skip_serializing)]
    pub c_index: usize,
}
