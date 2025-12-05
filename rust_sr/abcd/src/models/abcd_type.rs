use serde::Serialize;
#[derive(Debug, Clone, Copy, Serialize)]
pub enum ABCDType {
    Butterfly,
    Bat,
    Gartley,
    Crab,
    Shark,
    Standard, 
    Extended,
    None
}