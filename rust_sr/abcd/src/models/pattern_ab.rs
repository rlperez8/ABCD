use serde::Serialize;
use crate::models::pivot::Pivot;
#[derive(Debug, Clone, Serialize)]
pub struct PatternXAB {
    pub x: Pivot,
    pub a: Pivot,
    pub b: Pivot,
}