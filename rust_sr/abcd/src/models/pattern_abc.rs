use serde::Serialize;
use crate::models::pivot::Pivot;
#[derive(Debug, Clone, Serialize)]
pub struct PatternXABC {
    pub x: Pivot,
    pub a: Pivot,
    pub b: Pivot,
    pub c: Pivot,
}