use serde::Serialize;
use crate::models::pivot::Pivot;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PatternXA {
    pub x: Pivot,
    pub a: Pivot,
}
