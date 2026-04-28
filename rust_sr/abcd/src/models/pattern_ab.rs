use crate::models::pivot::Pivot;
use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PatternXAB {
    pub x: Pivot,
    pub a: Pivot,
    pub b: Pivot,
    #[serde(skip_serializing)]
    pub x_index: usize,
    #[serde(skip_serializing)]
    pub a_index: usize,
    #[serde(skip_serializing)]
    pub b_index: usize,
}
