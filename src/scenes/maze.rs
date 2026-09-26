use super::Scene;
use crate::glyphs::glyph;
use crate::grid::Grid;
use crate::theme::value_color;

/// 10 PRINT CHR$(205.5+RND(1)); — a slowly mutating diagonal maze.
pub struct Maze {
    rng: fastrand::Rng,
    slash: Vec<bool>,
}

impl Maze {
    pub fn new(cols: u32, rows: u32, seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let slash = (0..cols * rows).map(|_| rng.bool()).collect();
        Maze { rng, slash }
    }
}

impl Scene for Maze {
    fn update(&mut self, t: f32, _dt: f32, grid: &mut Grid) {
        let n = self.slash.len();
        for _ in 0..(n as f32 * 0.002).ceil() as usize {
            let i = self.rng.usize(0..n);
            self.slash[i] = !self.slash[i];
        }
        let (fwd, back) = (glyph(b'/'), glyph(b'\\'));
        let cols = grid.cols as usize;
        for (i, cell) in grid.cells.iter_mut().enumerate() {
            let (col, row) = (i % cols, i / cols);
            let v = 0.5 + 0.5 * ((col + row) as f32 * 0.1 - t).sin();
            *cell = [if self.slash[i] { fwd } else { back }, value_color(v)];
        }
    }
}
