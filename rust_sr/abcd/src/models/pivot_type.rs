use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize,PartialEq, Eq)]
pub enum PivotType {
    High,
    Low,
}
