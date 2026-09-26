use super::FieldFn;
use std::f32::consts::TAU;

/// Interference of three drifting circular wave sources.
pub struct Waves {
    aspect: f32,
    params: [(f32, f32, f32); 3], // (a, b, phase)
    pos: [(f32, f32); 3],
}

impl Waves {
    pub fn new(seed: u64, aspect: f32) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let mut r = |lo: f32, hi: f32| lo + rng.f32() * (hi - lo);
        let params = [(); 3].map(|_| (r(0.05, 0.2), r(0.05, 0.2), r(0.0, TAU)));
        Waves { aspect, params, pos: [(0.0, 0.0); 3] }
    }
}

impl FieldFn for Waves {
    fn prepare(&mut self, t: f32) {
        for (p, &(a, b, ph)) in self.pos.iter_mut().zip(&self.params) {
            *p = (
                0.5 * self.aspect + 0.35 * self.aspect * (t * a + ph).sin(),
                0.5 + 0.35 * (t * b + ph).cos(),
            );
        }
    }

    #[inline]
    fn sample(&self, x: f32, y: f32, t: f32) -> f32 {
        let mut acc = 0.0;
        for &(px, py) in &self.pos {
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            acc += 0.5 + 0.5 * (d * 40.0 - t * 2.0).sin();
        }
        acc / 3.0
    }
}
