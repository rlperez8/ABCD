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

pub fn find_harmonic_type(x: &f64, a: &f64, b: &f64, c: &f64, d: &f64) -> ABCDType {
    let tol = 0.00;

    let xa = (a - x).abs();
    let ab = (b - a).abs();
    let bc = (c - b).abs();
    let xd = (d - x).abs();

    let b_ratio = ab / xa;   
    let c_ratio = bc / ab;   
    let d_ratio = xd / xa;     

    // println!("=============");
    // println!("X: {}", x);
    // println!("A: {}", a);
    // println!("B: {}", b);
    // println!("C: {}", c);
    // println!("D: {}", d);
    // println!("B Ratio: {}", b_ratio);
    // println!("C Ratio: {}", c_ratio);
    // println!("D Ratio: {}", d_ratio);

    // Gartley
    if (b_ratio - 0.618).abs() < tol &&
       (0.382..=0.886).contains(&c_ratio) &&
       (d_ratio - 0.786).abs() < tol {
        return ABCDType::Gartley;
    }

    // Bat
    if (0.382..=0.50).contains(&b_ratio) &&
       (d_ratio - 0.886).abs() < tol {
        return ABCDType::Bat;
    }

    // Butterfly
    if (b_ratio - 0.786).abs() < tol &&
       (1.27..=1.618).contains(&d_ratio) {
        return ABCDType::Butterfly;
    }

    // Crab
    if (0.382..=0.618).contains(&b_ratio) &&
       (d_ratio - 1.618).abs() < tol {
        return ABCDType::Crab;
    }

    // Shark
    if (0.382..=0.618).contains(&b_ratio) &&
       (0.886..=1.13).contains(&d_ratio) {
        return ABCDType::Shark;
    }

    ABCDType::None
}