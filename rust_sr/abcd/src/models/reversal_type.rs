use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ReversalType {
    BullishKeyReversal,
    BearishKeyReversal,
    BullishEngulfing,
    BearishEngulfing,
    BullishOutsideReversal,
    BearishOutsideReversal,
    MorningStar,
    EveningStar,
    ThreeWhiteSoldiers,
    ThreeBlackCrows,
    Hammer,
    ShootingStar,
    None,
}
