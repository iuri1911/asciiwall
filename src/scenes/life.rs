use super::Scene;
use crate::glyphs::glyph;
use crate::grid::Grid;
use crate::theme::value_color;

const STEP: f32 = 0.1; // 10 generations per second
const GHOST: u8 = 5; // generations a dead cell stays visible
const MAX_GENERATIONS: u32 = 1500;

/// Conway's Game of Life on a torus with fading ghosts.
pub struct Life {
    rng: fastrand::Rng,
    cols: usize,
    rows: usize,
    alive: Vec<bool>,
    next: Vec<bool>,
    born: Vec<bool>,
    neighbors: Vec<u8>,
    dead_for: Vec<u8>,
    acc: f32,
    generation: u32,
}

impl Life {
    pub fn new(cols: u32, rows: u32, seed: u64) -> Self {
        let n = (cols * rows) as usize;
        let mut life = Life {
            rng: fastrand::Rng::with_seed(seed),
            cols: cols as usize,
            rows: rows as usize,
            alive: vec![false; n],
            next: vec![false; n],
            born: vec![false; n],
            neighbors: vec![0; n],
            dead_for: vec![u8::MAX; n],
            acc: 0.0,
            generation: 0,
        };
        life.reseed();
        life
    }

    fn reseed(&mut self) {
        for a in &mut self.alive {
            *a = self.rng.f32() < 0.25;
        }
        self.born.copy_from_slice(&self.alive);
        self.generation = 0;
    }

    fn step(&mut self) {
        let (w, h) = (self.cols, self.rows);
        let mut population = 0usize;
        for y in 0..h {
            let (ym, yp) = ((y + h - 1) % h * w, (y + 1) % h * w);
            let yc = y * w;
            for x in 0..w {
                let (xm, xp) = ((x + w - 1) % w, (x + 1) % w);
                let a = &self.alive;
                let n = a[ym + xm] as u8
                    + a[ym + x] as u8
                    + a[ym + xp] as u8
                    + a[yc + xm] as u8
                    + a[yc + xp] as u8
                    + a[yp + xm] as u8
                    + a[yp + x] as u8
                    + a[yp + xp] as u8;
                let i = yc + x;
                let was = a[i];
                let now = n == 3 || (was && n == 2);
                self.next[i] = now;
                self.neighbors[i] = n;
                self.born[i] = now && !was;
                self.dead_for[i] = if now {
                    0
                } else if was {
                    1
                } else {
                    self.dead_for[i].saturating_add(1)
                };
                population += now as usize;
            }
        }
        std::mem::swap(&mut self.alive, &mut self.next);
        self.generation += 1;
        if population * 50 < self.alive.len() || self.generation >= MAX_GENERATIONS {
            self.reseed();
        }
    }
}

impl Scene for Life {
    fn update(&mut self, _t: f32, dt: f32, grid: &mut Grid) {
        self.acc += dt;
        while self.acc >= STEP {
            self.acc -= STEP;
            self.step();
        }
        let hash = glyph(b'#');
        let dot = glyph(b'.');
        for (i, cell) in grid.cells.iter_mut().enumerate() {
            *cell = if self.alive[i] {
                if self.born[i] {
                    [hash, 7]
                } else {
                    [hash, value_color(self.neighbors[i] as f32 / 8.0)]
                }
            } else if self.dead_for[i] <= GHOST {
                [dot, 1]
            } else {
                [0, 0]
            };
        }
    }

    fn warmup_steps(&self) -> u32 {
        100
    }
}
