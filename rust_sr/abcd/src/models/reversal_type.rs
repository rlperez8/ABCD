use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub enum ReversalType {
    MorningStar,
    EveningStar,
    ThreeWhiteSoldiers,
    ThreeBlackCrows,
    Hammer,
    None,
}
