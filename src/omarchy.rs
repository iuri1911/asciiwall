use crate::config::{Config, home};
use crate::glyphs::Font;
use crate::gpu::GpuCell;
use crate::raster::{save_png, snapshot};
use crate::scenes;
use crate::theme::{Palette, theme_name};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static SNAPSHOT_LOCK: Mutex<()> = Mutex::new(());
static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn data_dir() -> PathBuf {
    home().join(".local/share/asciiwall")
}

pub fn gallery_dir() -> PathBuf {
    data_dir().join("gallery")
}

fn state_file() -> PathBuf {
    home().join(".local/state/asciiwall/current")
}

pub fn current_scene() -> Option<String> {
    std::fs::read_to_string(state_file()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn write_current_scene(id: &str) -> Result<()> {
    let f = state_file();
    std::fs::create_dir_all(f.parent().unwrap())?;
    std::fs::write(f, id)?;
    Ok(())
}

/// User backgrounds for the current theme first, then the theme's own.
pub fn background_dirs() -> Vec<PathBuf> {
    vec![
        home().join(".config/omarchy/backgrounds").join(theme_name()),
        home().join(".local/state/omarchy/current/theme/backgrounds"),
    ]
}

/// Largest monitor (width, height, scale); 4K@1 if hyprctl is unavailable.
pub fn static_target() -> (u32, u32, f32) {
    let parse = || -> Option<(u32, u32, f32)> {
        let out = Command::new("hyprctl").args(["-j", "monitors"]).output().ok()?;
        let mons: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        mons.as_array()?
            .iter()
            .filter_map(|m| {
                Some((m["width"].as_u64()? as u32, m["height"].as_u64()? as u32, m["scale"].as_f64()? as f32))
            })
            .max_by_key(|(w, h, _)| w * h)
    };
    parse().unwrap_or((3840, 2160, 1.0))
}

pub fn set_background(path: &Path) {
    match Command::new("omarchy-theme-bg-set").arg(path).status() {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("asciiwall: omarchy-theme-bg-set exited with {s}"),
        Err(e) => eprintln!("asciiwall: cannot run omarchy-theme-bg-set: {e}"),
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

/// Hand the ~30 MB snapshot/PNG buffers back to the OS; the daemon idles for 30 minutes.
fn release_memory() {
    // SAFETY: glibc malloc_trim has no preconditions.
    unsafe { libc::malloc_trim(0) };
}

/// Render `id` at the largest monitor's resolution and make it the Omarchy background.
pub fn apply_scene(cfg: &Config, font: &Font, id: &str, seed: u64, gpu: &GpuCell) -> Result<()> {
    // Two rapid scene requests must not race on snapshots or publish out of order.
    let _guard = SNAPSHOT_LOCK.lock();
    let scene = scenes::resolve(id)?;
    let palette = Palette::load();
    let (w, h, scale) = static_target();
    let img = snapshot(&scene, w, h, font, (cfg.font_size * scale).round(), &palette, seed, None, gpu)?;
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let name = format!(
        "current-{}-{}-{}.png",
        now_ms(),
        std::process::id(),
        SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let path = dir.join(&name);
    let previous = std::fs::read_link(home().join(".local/state/omarchy/current/background")).ok();
    save_png(&img, &path)?;
    set_background(&path);
    // The shell loads transition images asynchronously; rapid changes can leave
    // a queued image pending for several seconds. Retain the previous image and
    // all snapshots from the last minute until the next rotation.
    for e in std::fs::read_dir(&dir)?.flatten() {
        let n = e.file_name();
        let n = n.to_string_lossy();
        if n.starts_with("current-") && n.ends_with(".png") && n != name
            && previous.as_deref() != Some(e.path().as_path())
            && e.metadata().and_then(|m| m.modified()).ok()
                .and_then(|t| t.elapsed().ok()).is_some_and(|age| age.as_secs() >= 60)
        {
            let _ = std::fs::remove_file(e.path());
        }
    }
    write_current_scene(id)?;
    release_memory();
    Ok(())
}

pub fn gallery_theme() -> String {
    std::fs::read_to_string(gallery_dir().join(".theme")).map(|s| s.trim().to_string()).unwrap_or_default()
}

pub fn gallery_is_stale(cfg: &Config) -> bool {
    gallery_theme() != theme_name()
        || scenes::enabled_ids(&cfg.scenes).iter().any(|id| !gallery_dir().join(format!("{id}.png")).is_file())
}

/// Render 1080p thumbnails of every enabled scene for the background picker.
pub fn render_gallery(cfg: &Config, font: &Font, gpu: &GpuCell) -> Result<()> {
    let dir = gallery_dir();
    std::fs::create_dir_all(&dir)?;
    let palette = Palette::load();
    let ids = scenes::enabled_ids(&cfg.scenes);
    for id in &ids {
        let scene = scenes::resolve(id)?;
        let img = snapshot(&scene, 1920, 1080, font, cfg.font_size.round(), &palette, 1, None, gpu)?;
        save_png(&img, &dir.join(format!("{id}.png")))?;
    }
    for e in std::fs::read_dir(&dir)?.flatten() {
        let p = e.path();
        let stale = p.extension().is_some_and(|x| x == "png")
            && p.file_stem().is_some_and(|s| !ids.iter().any(|id| id.as_str() == s));
        if stale {
            let _ = std::fs::remove_file(p);
        }
    }
    std::fs::write(dir.join(".theme"), theme_name())?;
    release_memory();
    Ok(())
}
