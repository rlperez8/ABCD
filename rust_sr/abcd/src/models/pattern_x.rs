use serde::Serialize;
use crate::models::pivot::Pivot;
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PatternX {
    pub x: Pivot,
}
