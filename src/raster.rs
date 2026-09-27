use crate::glyphs::{Atlas, Font};
use crate::gpu::GpuCell;
use crate::grid::{Grid, layout};
use crate::scenes::{self, SceneId, Setup};
use crate::theme::Palette;
use anyhow::{Context, Result};
use image::RgbImage;
use rayon::prelude::*;
use std::path::Path;

pub const SNAPSHOT_DT: f32 = 1.0 / 30.0;

/// Rasterize a grid into an RGB image (CPU, one rayon task per pixel row).
pub fn render(grid: &Grid, atlas: &Atlas, palette: &Palette, width: u32, height: u32) -> RgbImage {
    let (_, _, off_x, off_y) = layout(width, height, atlas.cell_w, atlas.cell_h);
    let bg = palette.colors[0];
    // lut[slot][coverage] -> rgb, blending in sRGB byte space.
    let mut lut = vec![[0u8; 3]; 8 * 256];
    for (slot, fg) in palette.colors.iter().enumerate() {
        for cov in 0..256usize {
            lut[slot * 256 + cov] = std::array::from_fn(|c| {
                (bg[c] as i32 + (fg[c] as i32 - bg[c] as i32) * cov as i32 / 255) as u8
            });
        }
    }
    let (cw, ch) = (atlas.cell_w as usize, atlas.cell_h as usize);
    let aw = atlas.width() as usize;
    let grid_w = grid.cols as usize * cw;
    let grid_h = grid.rows as usize * ch;
    let (ox, oy) = (off_x as usize, off_y as usize);

    let mut buf = vec![0u8; width as usize * height as usize * 3];
    buf.par_chunks_mut(width as usize * 3)
        .enumerate()
        .for_each(|(y, line)| {
            line.as_chunks_mut::<3>().0.fill(bg);
            if y < oy || y >= oy + grid_h {
                return;
            }
            let gy = y - oy;
            let (row, ly) = (gy / ch, gy % ch);
            let cells = &grid.cells[row * grid.cols as usize..][..grid.cols as usize];
            let atlas_row = &atlas.coverage[ly * aw..][..aw];
            let out = &mut line[ox * 3..(ox + grid_w) * 3];
            for (cell, dst) in cells.iter().zip(out.chunks_exact_mut(cw * 3)) {
                let [g, slot] = *cell;
                if g == 0 {
                    continue;
                }
                let cov = &atlas_row[g as usize * cw..][..cw];
                let lut = &lut[slot as usize * 256..][..256];
                for (c, px) in cov.iter().zip(dst.as_chunks_mut::<3>().0) {
                    *px = lut[*c as usize];
                }
            }
        });
    RgbImage::from_raw(width, height, buf).unwrap()
}

/// Write a PNG atomically (temp file + rename) so readers never see a partial file.
pub fn save_png(img: &RgbImage, path: &Path) -> Result<()> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let tmp = path.with_extension("png.tmp");
    {
        let file = std::fs::File::create(&tmp)
            .with_context(|| format!("cannot write {}", tmp.display()))?;
        let enc = PngEncoder::new_with_quality(
            std::io::BufWriter::new(file),
            CompressionType::Fast,
            FilterType::Adaptive,
        );
        img.write_with_encoder(enc)?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Render one frame of a scene to an image. `time` defaults to the scene's
/// initial phase (zero for medusa, `seed % 1000` seconds otherwise).
/// `font_px` is the configured size; `scenes::atlas_for` adapts it per scene.
#[allow(clippy::too_many_arguments)]
pub fn snapshot(
    id: &SceneId,
    width: u32,
    height: u32,
    font: &Font,
    font_px: f32,
    palette: &Palette,
    seed: u64,
    time: Option<f32>,
    gpu: &GpuCell,
) -> Result<RgbImage> {
    let mut out = None;
    frames(
        id,
        width,
        height,
        font,
        font_px,
        palette,
        seed,
        time,
        gpu,
        1,
        SNAPSHOT_DT,
        |img| out = Some(img),
    )?;
    Ok(out.expect("one frame"))
}

/// Render `count` consecutive frames `dt` seconds apart, starting where
/// `snapshot` starts, and hand each one to `sink`.
#[allow(clippy::too_many_arguments)]
pub fn frames(
    id: &SceneId,
    width: u32,
    height: u32,
    font: &Font,
    font_px: f32,
    palette: &Palette,
    seed: u64,
    time: Option<f32>,
    gpu: &GpuCell,
    count: u32,
    dt: f32,
    mut sink: impl FnMut(RgbImage),
) -> Result<()> {
    let atlas = scenes::atlas_for(id, font, font_px, width, height);
    let (cols, rows, _, _) = layout(width, height, atlas.cell_w, atlas.cell_h);
    let aspect = (cols * atlas.cell_w) as f32 / (rows * atlas.cell_h) as f32;
    let setup = Setup {
        cols,
        rows,
        aspect,
        seed,
        atlas: &atlas,
        gpu,
    };
    let mut scene = scenes::build(id, &setup)?;
    let mut grid = Grid::new(cols, rows);
    let mut t = time.unwrap_or_else(|| scenes::start_time(id, seed));
    for _ in 0..scene.warmup_steps() {
        scene.update(t, SNAPSHOT_DT, &mut grid);
        t += SNAPSHOT_DT;
    }
    for i in 0..count {
        if i > 0 {
            t += dt;
        }
        scene.update(t, dt, &mut grid);
        sink(render(&grid, &atlas, palette, width, height));
    }
    Ok(())
}
