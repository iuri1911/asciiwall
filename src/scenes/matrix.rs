use super::Scene;
use crate::glyphs::glyph;
use crate::grid::Grid;
use crate::theme::value_color;

struct Drop {
    head: f32,
    speed: f32,
    len: f32,
}

/// Digital rain: falling glyph trails fading from highlight to the darkest ramp color.
pub struct Matrix {
    rng: fastrand::Rng,
    rows: u32,
    drops: Vec<Drop>,
    chars: Vec<u8>,
}

impl Matrix {
    pub fn new(cols: u32, rows: u32, seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let drops = (0..cols)
            .map(|_| {
                let mut d = spawn(&mut rng, rows);
                d.head = rng.f32() * (rows as f32 + d.len);
                d
            })
            .collect();
        let chars = (0..cols * rows).map(|_| glyph(rng.u8(33..=126))).collect();
        Matrix {
            rng,
            rows,
            drops,
            chars,
        }
    }
}

fn spawn(rng: &mut fastrand::Rng, rows: u32) -> Drop {
    Drop {
        head: -(rng.u32(0..rows.max(1)) as f32),
        speed: 6.0 + rng.f32() * 14.0,
        len: 6.0 + rng.f32() * 24.0,
    }
}

impl Scene for Matrix {
    fn update(&mut self, _t: f32, dt: f32, grid: &mut Grid) {
        let n = self.chars.len();
        for _ in 0..(n as f32 * 0.02).ceil() as usize {
            let i = self.rng.usize(0..n);
            self.chars[i] = glyph(self.rng.u8(33..=126));
        }
        let cols = grid.cols as usize;
        for (col, d) in self.drops.iter_mut().enumerate() {
            d.head += d.speed * dt;
            if d.head - d.len > self.rows as f32 {
                *d = spawn(&mut self.rng, self.rows);
            }
            for row in 0..grid.rows as usize {
                let i = row * cols + col;
                let k = d.head - row as f32;
                grid.cells[i] = if (0.0..1.0).contains(&k) {
                    [self.chars[i], 7]
                } else if k >= 1.0 && k < d.len {
                    [self.chars[i], value_color(1.0 - k / d.len)]
                } else {
                    [0, 0]
                };
            }
        }
    }

    fn warmup_steps(&self) -> u32 {
        300
    }
}
