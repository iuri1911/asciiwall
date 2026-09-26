use ab_glyph::{Font as _, FontArc, PxScale, point};
use anyhow::{Context, Result, bail};

/// Lazily-parsed font (outlines are read on demand, so a 10k-glyph Nerd Font costs only its file bytes).
pub type Font = FontArc;

pub const GLYPHS: u32 = 95; // printable ASCII 32..=126
pub const RAMP: &[u8] = b" .:-=+*#%@";

#[inline]
pub const fn glyph(b: u8) -> u8 {
    b - 32
}

/// Glyph index for a brightness value in [0,1].
#[inline]
pub fn ramp_glyph(v: f32) -> u8 {
    glyph(RAMP[(v.clamp(0.0, 1.0) * 9.0).round() as usize])
}

pub fn load_font(family: &str) -> Result<Font> {
    let out = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", &format!("{family}:style=Regular")])
        .output()
        .context("fc-match failed")?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        bail!("font not found: {family}");
    }
    let bytes = std::fs::read(&path).with_context(|| format!("font not found: {family}"))?;
    FontArc::try_from_vec(bytes).map_err(|e| anyhow::anyhow!("bad font {path}: {e}"))
}

/// One row of 95 monospace cells, 8-bit coverage.
pub struct Atlas {
    pub cell_w: u32,
    pub cell_h: u32,
    pub coverage: Vec<u8>,
}

impl Atlas {
    /// `px` is the em size in pixels.
    pub fn new(font: &Font, px: f32) -> Atlas {
        let k = px / font.units_per_em().unwrap_or(1000.0);
        let cell_w = (font.h_advance_unscaled(font.glyph_id('M')) * k).ceil().max(1.0) as u32;
        let ascent = font.ascent_unscaled() * k;
        let cell_h = (ascent - font.descent_unscaled() * k).ceil().max(1.0) as u32;
        let baseline = ascent.round();
        let scale = PxScale::from(font.height_unscaled() * k);
        let width = GLYPHS * cell_w;
        let mut coverage = vec![0u8; (width * cell_h) as usize];
        for i in 1..GLYPHS {
            let g = font.glyph_id((i as u8 + 32) as char).with_scale_and_position(scale, point(0.0, baseline));
            let Some(outline) = font.outline_glyph(g) else { continue };
            let b = outline.px_bounds();
            outline.draw(|gx, gy, c| {
                let (x, y) = (b.min.x as i32 + gx as i32, b.min.y as i32 + gy as i32);
                if x < 0 || y < 0 || x >= cell_w as i32 || y >= cell_h as i32 {
                    return;
                }
                let dst = (y as u32 * width + i * cell_w + x as u32) as usize;
                coverage[dst] = coverage[dst].max((c.clamp(0.0, 1.0) * 255.0).round() as u8);
            });
        }
        Atlas { cell_w, cell_h, coverage }
    }

    pub fn width(&self) -> u32 {
        GLYPHS * self.cell_w
    }

    /// Per-glyph ink in a 2x3 grid of cell regions (index = row*2 + col), each
    /// dimension normalized by its maximum over all glyphs. After Alex Harri,
    /// "ASCII characters are not pixels". Layout: 8 floats per glyph, last 2 zero.
    pub fn shapes(&self) -> Vec<[f32; 8]> {
        let (cw, ch, aw) = (self.cell_w as usize, self.cell_h as usize, self.width() as usize);
        let mut out = vec![[0f32; 8]; GLYPHS as usize];
        for (g, v) in out.iter_mut().enumerate() {
            for (j, d) in v.iter_mut().take(SHAPE_DIMS).enumerate() {
                let (x0, x1) = ((j % 2) * cw / 2, (j % 2 + 1) * cw / 2);
                let (y0, y1) = ((j / 2) * ch / 3, (j / 2 + 1) * ch / 3);
                let mut sum = 0u32;
                for y in y0..y1 {
                    let row = &self.coverage[y * aw + g * cw..][..cw];
                    sum += row[x0..x1].iter().map(|&c| c as u32).sum::<u32>();
                }
                *d = sum as f32 / (255 * (x1 - x0).max(1) * (y1 - y0).max(1)) as f32;
            }
        }
        for j in 0..SHAPE_DIMS {
            let max = out.iter().map(|v| v[j]).fold(0f32, f32::max).max(1e-6);
            out.iter_mut().for_each(|v| v[j] /= max);
        }
        out
    }
}

pub const SHAPE_DIMS: usize = 6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_ends() {
        assert_eq!(ramp_glyph(0.0), glyph(b' '));
        assert_eq!(ramp_glyph(1.0), glyph(b'@'));
    }
}
