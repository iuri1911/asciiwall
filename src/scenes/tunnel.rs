use super::FieldFn;

/// Infinite polar tunnel.
pub struct Tunnel {
    pub aspect: f32,
}

impl FieldFn for Tunnel {
    #[inline]
    fn sample(&self, x: f32, y: f32, t: f32) -> f32 {
        let (dx, dy) = (x - 0.5 * self.aspect, y - 0.5);
        let r = (dx * dx + dy * dy).sqrt().max(0.02);
        let a = dy.atan2(dx);
        (0.5 + 0.5 * (6.0 / r - t * 2.0).sin())
            * (0.5 + 0.5 * (a * 8.0 + t).sin())
            * (r * 2.0).clamp(0.0, 1.0)
    }
}
