//! Classic ASCII art shown as drawn: every character is kept, the piece is
//! centered and scaled to fit, and cells are widened to the proportions of
//! the font it was made in so the drawing is not squeezed. Color follows each
//! glyph's ink, and the picture gets the image scenes' slow light sweep.

use super::Setup;
use super::image::ImageScene;
use crate::glyphs::{Atlas, Font, em_metrics, glyph};
use crate::theme::value_color;

pub struct Spec {
    pub name: &'static str,
    pub art: &'static str,
    /// Cell width / height of the font the art was drawn in.
    pub cell_aspect: f32,
}

/// PsyBear's anime ASCII portraits (2007), CC BY-NC-SA 3.0, transcribed cell
/// by cell from the published images; see plates/README.md.
pub const SPECS: &[Spec] = &[
    Spec {
        // https://www.deviantart.com/psybear/art/Serial-Experiments-Lain-ASCII-65841237
        name: "lain",
        art: include_str!("plates/lain.txt"),
        cell_aspect: 0.6158,
    },
    Spec {
        // https://www.deviantart.com/psybear/art/Rei-Ayanami-ASCII-3-65844587
        name: "rei",
        art: include_str!("plates/rei.txt"),
        cell_aspect: 0.6153,
    },
];

/// Empty border around the piece, as a fraction of each screen side.
const MARGIN: f32 = 0.03;

pub fn spec(name: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.name == name)
}

fn size(art: &str) -> (usize, usize) {
    let cols = art.lines().map(str::len).max().unwrap_or(0);
    (cols.max(1), art.lines().count().max(1))
}

/// Largest atlas whose cells fit the whole piece on a `width`x`height` target.
pub fn atlas(spec: &Spec, font: &Font, width: u32, height: u32) -> Atlas {
    let (cols, rows) = size(spec.art);
    let (adv, line) = em_metrics(font);
    let avail_h = height as f32 * (1.0 - 2.0 * MARGIN) / rows as f32;
    let avail_w = width as f32 * (1.0 - 2.0 * MARGIN) / (cols as f32 * spec.cell_aspect);
    let cell_h = avail_h.min(avail_w).floor().max(4.0);
    // Shave a hair off so `ceil` in the atlas cannot round the line up a pixel.
    let px = (cell_h - 0.01) / line;
    let natural = (px * adv).ceil() as u32;
    let track = ((cell_h * spec.cell_aspect).round() as u32).saturating_sub(natural);
    Atlas::spaced(font, px, track)
}

pub fn build(spec: &Spec, s: &Setup) -> ImageScene {
    let (grid_cols, grid_rows) = (s.cols as usize, s.rows as usize);
    let (art_cols, art_rows) = size(spec.art);
    let ox = grid_cols.saturating_sub(art_cols) / 2;
    let oy = grid_rows.saturating_sub(art_rows) / 2;

    let ink = glyph_ink(s.atlas);
    let mut glyphs = vec![0u8; grid_cols * grid_rows];
    let mut slots = vec![0u8; grid_cols * grid_rows];
    for (r, line) in spec.art.lines().enumerate().take(grid_rows) {
        for (c, b) in line.bytes().enumerate().take(grid_cols) {
            if !(33..=126).contains(&b) {
                continue;
            }
            let i = (oy + r) * grid_cols + ox + c;
            let g = glyph(b);
            glyphs[i] = g;
            slots[i] = value_color(ink[g as usize]);
        }
    }
    ImageScene::from_cells(glyphs, slots, grid_cols)
}

/// Ink of each glyph relative to the darkest one, eased so sparse strokes
/// still get a visible color.
fn glyph_ink(atlas: &Atlas) -> Vec<f32> {
    let (cw, ch, aw) = (
        atlas.cell_w as usize,
        atlas.cell_h as usize,
        atlas.width() as usize,
    );
    let sums: Vec<f32> = (0..crate::glyphs::GLYPHS as usize)
        .map(|g| {
            (0..ch)
                .map(|y| {
                    atlas.coverage[y * aw + g * cw..][..cw]
                        .iter()
                        .map(|&c| c as f32)
                        .sum::<f32>()
                })
                .sum()
        })
        .collect();
    let max = sums.iter().copied().fold(1.0, f32::max);
    sums.iter().map(|s| (s / max).sqrt()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plates_are_printable_ascii() {
        for s in SPECS {
            assert!(
                s.art.bytes().all(|b| b == b'\n' || (32..=126).contains(&b)),
                "{} has bytes outside printable 7-bit ASCII",
                s.name
            );
        }
    }
}
