mod image;
mod life;
mod matrix;
mod maze;
mod noise;
mod pack;
mod plasma;
mod plate;
pub mod shader;
pub use pack::check;
mod tunnel;
mod waves;

use crate::glyphs::{Atlas, Font, ramp_glyph};
use crate::gpu::GpuCell;
use crate::grid::Grid;
use crate::theme::value_color;
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
    pub atlas: &'a Atlas,
    pub gpu: &'a GpuCell,
}

/// CPU patterns, then classic plates, then shader scenes (in `shader::SPECS` order).
pub const PATTERNS: [&str; 7] = [
    "waves", "plasma", "noise", "tunnel", "matrix", "life", "maze",
];

pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    PATTERNS
        .into_iter()
        .chain(plate::SPECS.iter().map(|s| s.name))
        .chain(shader::SPECS.iter().map(|s| s.name))
}
pub fn is_plate(id: &str) -> bool {
    plate::spec(id).is_some()
}

const IMAGE_PREFIX: &str = "image--";
const IMAGE_EXTS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

#[derive(Debug, Clone, PartialEq)]
pub enum SceneId {
    Pattern(&'static str),
    Image(PathBuf),
    Pack(String),
}
/// Medusa opens in view; other scenes retain their seed-derived starting phase.
pub fn start_time(id: &SceneId, seed: u64) -> f32 {
    if matches!(id, SceneId::Pattern("medusa")) {
        0.0
    } else {
        (seed % 1000) as f32
    }
}

impl SceneId {
    /// Config key for this scene's tempo.
    pub fn key(&self) -> String {
        match self {
            SceneId::Pattern(name) => (*name).to_string(),
            SceneId::Pack(id) => id.clone(),
            SceneId::Image(path) => format!(
                "{IMAGE_PREFIX}{}",
                path.file_stem().unwrap_or_default().to_string_lossy()
            ),
        }
    }
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
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut stems: Vec<String> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_image(p))
            .filter_map(|p| {
                p.file_stem()
                    .map(|s| format!("{IMAGE_PREFIX}{}", s.to_string_lossy()))
            })
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

pub fn resolve_in(id: &str, dirs: &[PathBuf], pack_roots: &[PathBuf]) -> Result<SceneId> {
    if let Some(p) = builtin_names().find(|p| *p == id) {
        return Ok(SceneId::Pattern(p));
    }
    if let Some(pack) = pack::find_in(id, pack_roots)? {
        return Ok(SceneId::Pack(pack.id));
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
    resolve_in(id, &crate::omarchy::background_dirs(), &pack::roots())
}

/// Every scene id available right now.
pub fn all_ids() -> Vec<String> {
    let mut ids: Vec<String> = builtin_names().map(String::from).collect();
    ids.extend(pack::ids());
    ids.extend(image_ids_in(&crate::omarchy::background_dirs()));
    ids
}

/// Scene ids enabled by config. `"images"` expands to theme wallpapers,
/// `"shaders"` to built-in shader scenes, `"plates"` to classic ASCII art,
/// `"packs"` to installed packs.
pub fn enabled_ids(scenes: &[String]) -> Vec<String> {
    let images = image_ids_in(&crate::omarchy::background_dirs());
    let packs = pack::ids();
    let mut out: Vec<String> = Vec::new();
    for s in scenes {
        let expanded: Vec<String> = if s == "images" {
            images.clone()
        } else if s == "shaders" {
            shader::SPECS.iter().map(|s| s.name.to_string()).collect()
        } else if s == "plates" {
            plate::SPECS.iter().map(|s| s.name.to_string()).collect()
        } else if s == "packs" {
            packs.clone()
        } else if builtin_names().any(|p| p == s) || images.contains(s) || packs.contains(s) {
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

/// Cell size relative to the configured font. Pictures get smaller cells than
/// patterns: their detail is limited by the number of cells, a pattern's isn't.
pub fn font_scale(id: &SceneId) -> f32 {
    match id {
        SceneId::Image(_) => 0.7,
        SceneId::Pack(name) => pack::font_scale_of(name),
        SceneId::Pattern(_) => 1.0,
    }
}

/// Glyph atlas a scene is drawn with on a `width`x`height` pixel target, from
/// the configured size `base_px` in physical pixels. Plates ignore the
/// configured size: the whole piece must fit the screen.
pub fn atlas_for(id: &SceneId, font: &Font, base_px: f32, width: u32, height: u32) -> Atlas {
    if let SceneId::Pattern(name) = id
        && let Some(spec) = plate::spec(name)
    {
        return plate::atlas(spec, font, width, height);
    }
    Atlas::new(font, (base_px * font_scale(id)).round())
}

/// User tempo times the scene's authored pulse, clamped so a pack cannot
/// request a clock the frame cap cannot absorb.
pub fn clock_scale(scene: &str, cfg: &crate::config::Config) -> f32 {
    let pulse = if builtin_names().any(|n| n == scene) {
        1.0
    } else {
        pack::pulse_of(scene)
    };
    (pulse * cfg.tempo_of(scene).factor()).clamp(0.25, 4.0)
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
        SceneId::Pattern(name) => match (plate::spec(name), shader::spec(name)) {
            (Some(spec), _) => Box::new(plate::build(spec, s)),
            (None, Some(spec)) => Box::new(shader::ShaderScene::new(spec, s)?),
            (None, None) => bail!("unknown scene: {name}"),
        },
        SceneId::Pack(id) => {
            let Some(loaded) = pack::find(id)? else {
                bail!("unknown scene: {id}");
            };
            match loaded.kind {
                pack::Kind::Shader => {
                    let src = pack::read_shader(&loaded)?;
                    let spec = pack::shader_view(&loaded, &src)?;
                    Box::new(shader::ShaderScene::new(&spec, s)?)
                }
                pack::Kind::Wallpaper => {
                    let Some(path) = loaded.wallpaper.clone() else {
                        bail!("pack {id} has no wallpaper");
                    };
                    Box::new(image::ImageScene::new(&path, s)?)
                }
            }
        }
        SceneId::Image(path) => Box::new(image::ImageScene::new(path, s)?),
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
        assert_eq!(
            resolve_in("plasma", &dirs, &[]).unwrap(),
            SceneId::Pattern("plasma")
        );
        assert_eq!(
            resolve_in("image--1-on-pole", &dirs, &[]).unwrap(),
            SceneId::Image(dir.join("1-on-pole.jpg"))
        );
        assert!(resolve_in("bogus", &dirs, &[]).is_err());
        assert!(resolve_in("image--missing", &dirs, &[]).is_err());
        assert_eq!(image_ids_in(&dirs), vec!["image--1-on-pole".to_string()]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn clock_keeps_medium_at_the_written_pace() {
        let cfg = crate::config::Config::default();
        assert_eq!(clock_scale("koi", &cfg), 1.0);
        let mut slow = cfg.clone();
        slow.set_tempo("koi", crate::config::Tempo::Low);
        assert_eq!(clock_scale("koi", &slow), 0.5);
        slow.set_tempo("koi", crate::config::Tempo::High);
        assert_eq!(clock_scale("koi", &slow), 2.0);
    }
}
