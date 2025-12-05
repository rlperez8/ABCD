pub mod db;       // makes db.rs available
pub mod abcd_;    // makes abcd_.rs available

// optional: re-export types to simplify usage
pub use db::{Database, Candle};
pub use abcd_::ABCD;