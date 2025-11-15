// src/lib.rs

// Expose each file (module) in your tools crate:
pub mod db_utils;

// pub mod api_utils;
// pub mod file_utils;

// Optionally, you can also add common helpers directly here:
pub fn print_banner() {
    println!("🧰 Tools crate loaded successfully!");
}