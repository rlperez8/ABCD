use crate::models::harmonic_types::HarmonicType;

pub struct ScatterPlot {
    pub accuracy: f64, 
    pub return_pct: f64,
    pub harmonic_type: HarmonicType,
}   



impl ScatterPlot {

    pub fn new()-> Self {
        ScatterPlot {
            accuracy: 0.0,
            return_pct: 0.0,
            harmonic_type: HarmonicType::Bat, 
        }
    }

    pub fn gather_data(&self, patterns: Vec<Trade>)-> Vec<ScatterPlot> {

        let mut scatter_data: Vec<ScatterPlot> = Vec::new();

        for pattern in patterns.iter() {
            let mut scatter_point = ScatterPlot::new();
            scatter_point.accuracy = pattern.pattern_accuracy;
            scatter_point.return_pct = pattern.return_pct;
            scatter_point.harmonic_type = pattern.harmonic_type.clone();
            scatter_data.push(scatter_point);
        }

        scatter_data



    }

}