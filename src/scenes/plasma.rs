use super::FieldFn;

/// Classic demoscene sine plasma.
pub struct Plasma;

impl FieldFn for Plasma {
    #[inline]
    fn sample(&self, x: f32, y: f32, t: f32) -> f32 {
        let (x, y) = (x * 10.0, y * 10.0);
        let cx = x + 5.0 * (t / 5.0).sin();
        let cy = y + 5.0 * (t / 3.0).cos();
        let s = (x + t).sin()
            + ((y + t) / 2.0).sin()
            + ((x + y + t) / 2.0).sin()
            + ((cx * cx + cy * cy).sqrt() + t).sin();
        (s + 4.0) / 8.0
    }
}
