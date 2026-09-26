use super::Scene;
use crate::glyphs::ramp_glyph;
use crate::grid::Grid;
use crate::theme::Palette;
use anyhow::{Context, Result};
use std::path::Path;

/// A theme wallpaper converted to ASCII, colors snapped to the palette, with a slow shimmer.
pub struct ImageScene {
    lum: Vec<f32>,
    color: Vec<u8>,
}

impl ImageScene {
    pub fn new(path: &Path, cols: u32, rows: u32, aspect: f32, palette: &Palette) -> Result<Self> {
        let img = image::open(path).with_context(|| format!("cannot load image: {}", path.display()))?;
        let (w, h) = (img.width() as f32, img.height() as f32);
        let (cw, ch) = if w / h > aspect { (h * aspect, h) } else { (w, w / aspect) };
        let img = img
            .crop_imm(((w - cw) / 2.0) as u32, ((h - ch) / 2.0) as u32, cw as u32, ch as u32)
            .thumbnail_exact(cols, rows)
            .into_rgb8();

        let mut lum: Vec<f32> = img
            .pixels()
            .map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0)
            .collect();
        let mut sorted = lum.clone();
        sorted.sort_by(f32::total_cmp);
        let pct = |q: f32| sorted[((sorted.len() - 1) as f32 * q) as usize];
        let (lo, hi) = (pct(0.02), pct(0.98));
        let span = (hi - lo).max(1e-3);
        for l in &mut lum {
            *l = ((*l - lo) / span).clamp(0.0, 1.0);
        }
        let color = img.pixels().map(|p| palette.nearest(p.0)).collect();
        Ok(ImageScene { lum, color })
    }
}

impl Scene for ImageScene {
    fn update(&mut self, t: f32, _dt: f32, grid: &mut Grid) {
        let cols = grid.cols as usize;
        for (i, cell) in grid.cells.iter_mut().enumerate() {
            let (col, row) = ((i % cols) as f32, (i / cols) as f32);
            let v = self.lum[i] + 0.15 * (col * 0.08 + row * 0.05 - t * 1.5).sin();
            *cell = [ramp_glyph(v), self.color[i]];
        }
    }
}
