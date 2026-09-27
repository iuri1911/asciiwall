use super::{Scene, Setup};
use crate::glyphs::{SHAPE_DIMS, glyph};
use crate::grid::Grid;
use crate::theme::value_color;
use anyhow::{Context, Result};
use image::imageops::FilterType;
use std::path::Path;

/// Harri's directional contrast: how hard a sample is darkened by a brighter
/// neighbor just across the cell border, which keeps silhouettes crisp.
const DIRECTIONAL: f32 = 1.5;
/// Harri's global contrast inside one cell, which sharpens its shape.
const GLOBAL: f32 = 1.3;
/// A light sweep crosses the picture once per period, recoloring cells but
/// never changing a glyph, so the art itself stays still.
const SWEEP_PERIOD: f32 = 45.0;
const SWEEP_SECS: f32 = 8.0;
const SWEEP_WIDTH: f32 = 0.06;
/// Local contrast (unsharp mask over a box of +-BLUR samples): folds inside a
/// bright jacket or a cloud survive as glyph shapes instead of a flat `@` mass.
const DETAIL: f32 = 0.5;
const BLUR: usize = 8;
/// Stretched brightness below this is empty screen: clean negative space
/// instead of a haze of `.` and `'` over dark areas.
const BLACK: f32 = 0.15;

/// Glyphs used for pictures: marks and a few letters that read as shapes. The
/// rest of the alphabet (and near-identical dense glyphs like `$&Q`) turns
/// textured areas into text soup.
const PICTURE_GLYPHS: &[u8] = b" .,:;'`\"^~-_=+*!/\\|()<>[]#%@0oOXYTLJVcvxzil17";

/// A theme wallpaper as ASCII art: the picture is sampled in 2x3 regions per
/// cell and each cell gets the glyph whose ink has that shape (after Alex
/// Harri, "ASCII characters are not pixels"); brightness picks the color.
pub struct ImageScene {
    glyph: Vec<u8>,
    slot: Vec<u8>,
    /// Position of each cell along the sweep direction, 0..1.
    diag: Vec<f32>,
}

impl ImageScene {
    pub fn new(path: &Path, s: &Setup) -> Result<Self> {
        let (cols, rows) = (s.cols as usize, s.rows as usize);
        let (sw, sh) = (cols * 2, rows * 3);
        let img =
            image::open(path).with_context(|| format!("cannot load image: {}", path.display()))?;
        let (w, h) = (img.width() as f32, img.height() as f32);
        let (cw, ch) = if w / h > s.aspect {
            (h * s.aspect, h)
        } else {
            (w, w / s.aspect)
        };
        let img = img
            .crop_imm(
                ((w - cw) / 2.0) as u32,
                ((h - ch) / 2.0) as u32,
                cw as u32,
                ch as u32,
            )
            .resize_exact(sw as u32, sh as u32, FilterType::Triangle)
            .into_rgb8();

        let lum: Vec<f32> = img
            .pixels()
            .map(|p| (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0)
            .collect();
        let mut sorted = lum.clone();
        sorted.sort_by(f32::total_cmp);
        // Light-paper drawings are inked in reverse: on a dark screen the lines
        // become glyphs and the paper stays empty, instead of a wall of `@`.
        let lum: Vec<f32> = if sorted[sorted.len() / 2] > 0.5 {
            lum.iter().map(|l| 1.0 - l).collect()
        } else {
            lum
        };
        let mut lum = unsharp(&lum, sw, sh);
        sorted.clone_from(&lum);
        sorted.sort_by(f32::total_cmp);
        let pct = |q: f32| sorted[((sorted.len() - 1) as f32 * q) as usize];
        let (lo, hi) = (pct(0.01), pct(0.99));
        let span = (hi - lo).max(1e-3);
        for l in &mut lum {
            *l = (((*l - lo) / span - BLACK) / (1.0 - BLACK)).clamp(0.0, 1.0);
        }

        let shapes = s.atlas.shapes();
        let allowed: Vec<(u8, [f32; 8])> = PICTURE_GLYPHS
            .iter()
            .map(|&b| (glyph(b), shapes[glyph(b) as usize]))
            .collect();
        let mut glyph = Vec::with_capacity(cols * rows);
        let mut slot = Vec::with_capacity(cols * rows);
        let mut diag = Vec::with_capacity(cols * rows);
        let at = |x: usize, y: usize| lum[y * sw + x];
        for cy in 0..rows {
            for cx in 0..cols {
                let mut v = [0f32; SHAPE_DIMS];
                let mut raw = 0f32;
                for (j, vj) in v.iter_mut().enumerate() {
                    let (x, y) = (cx * 2 + (j & 1), cy * 3 + j / 2);
                    let own = at(x, y);
                    raw = raw.max(own);
                    // Neighbors across the cell border only (never inside the same cell).
                    let mut m = own;
                    if j & 1 == 0 && x > 0 {
                        m = m.max(at(x - 1, y));
                    }
                    if j & 1 == 1 && x + 1 < sw {
                        m = m.max(at(x + 1, y));
                    }
                    if j / 2 == 0 && y > 0 {
                        m = m.max(at(x, y - 1));
                    }
                    if j / 2 == 2 && y + 1 < sh {
                        m = m.max(at(x, y + 1));
                    }
                    *vj = if m > 0.0 {
                        (own / m).powf(DIRECTIONAL) * m
                    } else {
                        0.0
                    };
                }
                let top = v.iter().copied().fold(0.0, f32::max);
                if top > 0.0 {
                    for vj in &mut v {
                        *vj = (*vj / top).powf(GLOBAL) * top;
                    }
                }
                let g = nearest(&allowed, &v);
                glyph.push(g);
                slot.push(value_color(raw));
                diag.push(0.75 * cx as f32 / cols as f32 + 0.25 * cy as f32 / rows as f32);
            }
        }
        Ok(ImageScene { glyph, slot, diag })
    }

    /// A still picture from ready glyphs and colors, with the same light sweep.
    pub(super) fn from_cells(glyph: Vec<u8>, slot: Vec<u8>, cols: usize) -> Self {
        let rows = glyph.len() / cols.max(1);
        let diag = (0..glyph.len())
            .map(|i| {
                0.75 * (i % cols) as f32 / cols as f32 + 0.25 * (i / cols) as f32 / rows as f32
            })
            .collect();
        ImageScene { glyph, slot, diag }
    }
}

/// `l + DETAIL * (l - box_blur(l))`, with a separable running-sum box blur.
fn unsharp(l: &[f32], w: usize, h: usize) -> Vec<f32> {
    let blur_line = |src: &[f32], n: usize, stride: usize, off: usize, dst: &mut [f32]| {
        let mut sum = 0.0;
        let at = |i: usize| src[off + i * stride];
        for i in 0..BLUR.min(n) {
            sum += at(i);
        }
        for i in 0..n {
            if i + BLUR < n {
                sum += at(i + BLUR);
            }
            if i > BLUR {
                sum -= at(i - BLUR - 1);
            }
            let count = (i + BLUR).min(n - 1) - i.saturating_sub(BLUR) + 1;
            dst[off + i * stride] = sum / count as f32;
        }
    };
    let mut rows = vec![0.0; l.len()];
    for y in 0..h {
        blur_line(l, w, 1, y * w, &mut rows);
    }
    let mut blur = vec![0.0; l.len()];
    for x in 0..w {
        blur_line(&rows, h, w, x, &mut blur);
    }
    l.iter()
        .zip(&blur)
        .map(|(v, b)| v + DETAIL * (v - b))
        .collect()
}

/// Allowed glyph whose 2x3 ink vector is closest to `v`.
fn nearest(allowed: &[(u8, [f32; 8])], v: &[f32; SHAPE_DIMS]) -> u8 {
    let mut best = (0u8, f32::MAX);
    for (g, s) in allowed {
        let d: f32 = s.iter().zip(v).map(|(a, b)| (a - b) * (a - b)).sum();
        if d < best.1 {
            best = (*g, d);
        }
    }
    best.0
}

impl Scene for ImageScene {
    fn update(&mut self, t: f32, _dt: f32, grid: &mut Grid) {
        let phase = t.rem_euclid(SWEEP_PERIOD);
        // Band center runs a little past both edges so it enters and leaves cleanly.
        let band = (phase < SWEEP_SECS)
            .then(|| phase / SWEEP_SECS * (1.0 + 4.0 * SWEEP_WIDTH) - 2.0 * SWEEP_WIDTH);
        for (i, cell) in grid.cells.iter_mut().enumerate() {
            let mut slot = self.slot[i];
            if let Some(b) = band
                && (self.diag[i] - b).abs() < SWEEP_WIDTH
            {
                slot = (slot + 1).min(7);
            }
            *cell = [self.glyph[i], slot];
        }
    }
}
