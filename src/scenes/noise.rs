use super::FieldFn;

/// Drifting clouds: 3-octave fBm over Ken Perlin's improved noise.
pub struct Noise {
    p: [u8; 512],
}

impl Noise {
    pub fn new(seed: u64) -> Self {
        let mut perm: Vec<u8> = (0..=255).collect();
        fastrand::Rng::with_seed(seed).shuffle(&mut perm);
        let mut p = [0u8; 512];
        for (i, v) in p.iter_mut().enumerate() {
            *v = perm[i & 255];
        }
        Noise { p }
    }

    #[inline]
    fn perlin(&self, x: f32, y: f32, z: f32) -> f32 {
        let (fx, fy, fz) = (x.floor(), y.floor(), z.floor());
        let (xi, yi, zi) = (fx as i32 as usize & 255, fy as i32 as usize & 255, fz as i32 as usize & 255);
        let (x, y, z) = (x - fx, y - fy, z - fz);
        let (u, v, w) = (fade(x), fade(y), fade(z));
        let p = &self.p;
        let a = p[xi] as usize + yi;
        let (aa, ab) = (p[a] as usize + zi, p[a + 1] as usize + zi);
        let b = p[xi + 1] as usize + yi;
        let (ba, bb) = (p[b] as usize + zi, p[b + 1] as usize + zi);
        lerp(
            w,
            lerp(
                v,
                lerp(u, grad(p[aa], x, y, z), grad(p[ba], x - 1.0, y, z)),
                lerp(u, grad(p[ab], x, y - 1.0, z), grad(p[bb], x - 1.0, y - 1.0, z)),
            ),
            lerp(
                v,
                lerp(u, grad(p[aa + 1], x, y, z - 1.0), grad(p[ba + 1], x - 1.0, y, z - 1.0)),
                lerp(u, grad(p[ab + 1], x, y - 1.0, z - 1.0), grad(p[bb + 1], x - 1.0, y - 1.0, z - 1.0)),
            ),
        )
    }
}

#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

#[inline]
fn grad(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

impl FieldFn for Noise {
    #[inline]
    fn sample(&self, x: f32, y: f32, t: f32) -> f32 {
        let (mut amp, mut freq, mut sum) = (1.0, 1.0, 0.0);
        for _ in 0..3 {
            sum += amp * self.perlin(x * 3.0 * freq, y * 3.0 * freq, t * 0.08 * freq);
            amp *= 0.5;
            freq *= 2.0;
        }
        (0.5 + sum * 0.8).clamp(0.0, 1.0)
    }
}
