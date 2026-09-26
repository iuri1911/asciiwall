mod image;
mod life;
mod matrix;
mod maze;
mod noise;
mod plasma;
pub mod shader;
mod tunnel;
mod waves;

use crate::glyphs::{Atlas, ramp_glyph};
use crate::gpu::GpuCell;
use crate::grid::Grid;
use crate::theme::{Palette, value_color};
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

pub trait Scene {
    fn update(&mut self, t: f32, dt: f32, grid: &mut Grid);
    fn warmup_steps(&self) -> u32 {
        0
    }
    /// Scenes computed on the GPU: live mode encodes them directly instead of `update`.
    fn gpu(&self) -> Option<&shader::ShaderScene> {
        None
    }
}

/// Everything a scene is built against.
pub struct Setup<'a> {
    pub cols: u32,
    pub rows: u32,
    /// Grid pixel width / pixel height.
    pub aspect: f32,
    pub seed: u64,
    pub palette: &'a Palette,
    pub atlas: &'a Atlas,
    pub gpu: &'a GpuCell,
}

/// CPU patterns, then shader scenes (in `shader::SPECS` order).
pub const PATTERNS: [&str; 7] = ["waves", "plasma", "noise", "tunnel", "matrix", "life", "maze"];

pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    PATTERNS.into_iter().chain(shader::SPECS.iter().map(|s| s.name))
}

const IMAGE_PREFIX: &str = "image--";
const IMAGE_EXTS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

#[derive(Debug, Clone, PartialEq)]
pub enum SceneId {
    Pattern(&'static str),
    Image(PathBuf),
}

fn is_image(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Image scene ids found in `dirs`, first directory wins on duplicate stems.
pub fn image_ids_in(dirs: &[PathBuf]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        let mut stems: Vec<String> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_image(p))
            .filter_map(|p| p.file_stem().map(|s| format!("{IMAGE_PREFIX}{}", s.to_string_lossy())))
            .collect();
        stems.sort();
        for s in stems {
            if !ids.contains(&s) {
                ids.push(s);
            }
        }
    }
    ids
}

pub fn resolve_in(id: &str, dirs: &[PathBuf]) -> Result<SceneId> {
    if let Some(p) = builtin_names().find(|p| *p == id) {
        return Ok(SceneId::Pattern(p));
    }
    if let Some(stem) = id.strip_prefix(IMAGE_PREFIX) {
        for dir in dirs {
            for ext in IMAGE_EXTS {
                let p = dir.join(format!("{stem}.{ext}"));
                if p.is_file() {
                    return Ok(SceneId::Image(p));
                }
            }
        }
    }
    bail!("unknown scene: {id}")
}

pub fn resolve(id: &str) -> Result<SceneId> {
    resolve_in(id, &crate::omarchy::background_dirs())
}

/// Every scene id available right now.
pub fn all_ids() -> Vec<String> {
    let mut ids: Vec<String> = builtin_names().map(String::from).collect();
    ids.extend(image_ids_in(&crate::omarchy::background_dirs()));
    ids
}

/// Scene ids enabled by config ("images" expands to every image scene, "shaders" to every shader scene).
pub fn enabled_ids(scenes: &[String]) -> Vec<String> {
    let images = image_ids_in(&crate::omarchy::background_dirs());
    let mut out: Vec<String> = Vec::new();
    for s in scenes {
        let expanded: Vec<String> = if s == "images" {
            images.clone()
        } else if s == "shaders" {
            shader::SPECS.iter().map(|s| s.name.to_string()).collect()
        } else if builtin_names().any(|p| p == s) || images.contains(s) {
            vec![s.clone()]
        } else {
            eprintln!("asciiwall: ignoring unknown scene in config: {s}");
            vec![]
        };
        for e in expanded {
            if !out.contains(&e) {
                out.push(e);
            }
        }
    }
    if out.is_empty() {
        out = builtin_names().map(String::from).collect();
    }
    out
}

pub fn build(id: &SceneId, s: &Setup) -> Result<Box<dyn Scene>> {
    let (cols, rows, aspect, seed) = (s.cols, s.rows, s.aspect, s.seed);
    Ok(match id {
        SceneId::Pattern("waves") => Box::new(Field::new(aspect, waves::Waves::new(seed, aspect))),
        SceneId::Pattern("plasma") => Box::new(Field::new(aspect, plasma::Plasma)),
        SceneId::Pattern("noise") => Box::new(Field::new(aspect, noise::Noise::new(seed))),
        SceneId::Pattern("tunnel") => Box::new(Field::new(aspect, tunnel::Tunnel { aspect })),
        SceneId::Pattern("matrix") => Box::new(matrix::Matrix::new(cols, rows, seed)),
        SceneId::Pattern("life") => Box::new(life::Life::new(cols, rows, seed)),
        SceneId::Pattern("maze") => Box::new(maze::Maze::new(cols, rows, seed)),
        SceneId::Pattern(name) => match shader::spec(name) {
            Some(spec) => Box::new(shader::ShaderScene::new(spec, s)?),
            None => bail!("unknown scene: {name}"),
        },
        SceneId::Image(path) => Box::new(image::ImageScene::new(path, cols, rows, aspect, s.palette)?),
    })
}

/// A stateless scalar field sampled per cell: `v = f(x, y, t)`.
trait FieldFn {
    /// Called once per frame before sampling.
    fn prepare(&mut self, _t: f32) {}
    fn sample(&self, x: f32, y: f32, t: f32) -> f32;
}

struct Field<F: FieldFn> {
    aspect: f32,
    f: F,
}

impl<F: FieldFn> Field<F> {
    fn new(aspect: f32, f: F) -> Self {
        Field { aspect, f }
    }
}

impl<F: FieldFn> Scene for Field<F> {
    fn update(&mut self, t: f32, _dt: f32, grid: &mut Grid) {
        self.f.prepare(t);
        let sx = self.aspect / grid.cols as f32;
        let sy = 1.0 / grid.rows as f32;
        let cols = grid.cols as usize;
        for (row, line) in grid.cells.chunks_exact_mut(cols).enumerate() {
            let y = row as f32 * sy;
            for (col, cell) in line.iter_mut().enumerate() {
                let v = self.f.sample(col as f32 * sx, y, t);
                *cell = [ramp_glyph(v), value_color(v)];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_patterns_and_images() {
        let dir = std::env::temp_dir().join(format!("asciiwall-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("1-on-pole.jpg"), b"").unwrap();
        let dirs = vec![dir.clone()];
        assert_eq!(resolve_in("plasma", &dirs).unwrap(), SceneId::Pattern("plasma"));
        assert_eq!(
            resolve_in("image--1-on-pole", &dirs).unwrap(),
            SceneId::Image(dir.join("1-on-pole.jpg"))
        );
        assert!(resolve_in("bogus", &dirs).is_err());
        assert!(resolve_in("image--missing", &dirs).is_err());
        assert_eq!(image_ids_in(&dirs), vec!["image--1-on-pole".to_string()]);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
