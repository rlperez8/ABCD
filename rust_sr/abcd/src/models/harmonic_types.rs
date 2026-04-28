use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]

pub enum HarmonicType {
    Butterfly,
    Bat,
    AlternateBat,
    Gartley,
    Crab,
    DeepCrab,
    Shark,
    None,
}

impl Default for HarmonicType {
    fn default() -> Self {
        Self::None
    }
}
