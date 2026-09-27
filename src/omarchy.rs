use crate::config::{Config, home};
use crate::glyphs::Font;
use crate::gpu::GpuCell;
use crate::raster::{save_png, snapshot};
use crate::scenes;
use crate::theme::{Palette, theme_name};
use anyhow::Result;
use parking_lot::Mutex;
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::Command;
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
    std::fs::read_to_string(state_file())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_current_scene(id: &str) -> Result<()> {
    let f = state_file();
    std::fs::create_dir_all(f.parent().unwrap())?;
    std::fs::write(f, id)?;
    Ok(())
}

/// Omarchy's current-background symlink: what the shell and the lock screen show.
pub fn background_link() -> PathBuf {
    home().join(".local/state/omarchy/current/background")
}

/// The last non-asciiwall background, so turning asciiwall off can put it back.
fn stock_record() -> PathBuf {
    home().join(".local/state/asciiwall/stock-background")
}

fn record_stock_background(image: &Path) -> std::io::Result<()> {
    let f = stock_record();
    std::fs::create_dir_all(f.parent().unwrap())?;
    let tmp = f.with_extension("tmp");
    std::fs::write(&tmp, image.as_os_str().as_bytes())?;
    std::fs::rename(&tmp, &f)
}

/// True when `image` is a file directly inside one of `dirs`, the depth Omarchy's own
/// background scripts search. Compared after resolving symlinks because
/// omarchy-theme-bg-set links the realpath.
fn inside_any(image: &Path, dirs: &[PathBuf]) -> bool {
    let Ok(image) = image.canonicalize() else {
        return false;
    };
    image.is_file()
        && dirs
            .iter()
            .filter_map(|d| d.canonicalize().ok())
            .any(|d| image.parent() == Some(d.as_path()))
}

/// Unless a non-asciiwall image already is the background, hand it back to Omarchy:
/// the image asciiwall last replaced when it belongs to the current theme (a theme
/// change since then makes it stale), otherwise `omarchy-theme-bg-next`, which starts
/// at the theme's first image when the link is not one of them. Then drop snapshots
/// nothing shows.
pub fn restore_stock_background() {
    let stock_shown = std::fs::read_link(background_link())
        .is_ok_and(|p| !p.starts_with(data_dir()) && p.is_file());
    if !stock_shown {
        let recorded = std::fs::read(stock_record())
            .ok()
            .map(|b| PathBuf::from(OsString::from_vec(b)));
        match recorded.filter(|p| inside_any(p, &background_dirs())) {
            Some(p) => set_background(&p),
            None => match Command::new("omarchy-theme-bg-next").status() {
                Ok(s) if s.success() => {}
                Ok(s) => eprintln!("asciiwall: omarchy-theme-bg-next exited with {s}"),
                Err(e) => eprintln!("asciiwall: cannot run omarchy-theme-bg-next: {e}"),
            },
        }
    }
    let shown = std::fs::read_link(background_link()).ok();
    if let Err(e) = prune_snapshots(shown.as_deref().as_slice())
        && e.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("asciiwall: cannot prune snapshots: {e}");
    }
}

/// Delete `current-*.png` snapshots except `keep`. The shell loads transition images
/// asynchronously and rapid changes can leave one pending for several seconds, so
/// anything written in the last minute survives until the next call.
fn prune_snapshots(keep: &[&Path]) -> std::io::Result<()> {
    for e in std::fs::read_dir(data_dir())?.flatten() {
        let path = e.path();
        let n = e.file_name();
        let n = n.to_string_lossy();
        if n.starts_with("current-")
            && n.ends_with(".png")
            && !keep.contains(&path.as_path())
            && e.metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age.as_secs() >= 60)
        {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

/// User backgrounds for the current theme first, then the theme's own.
pub fn background_dirs() -> Vec<PathBuf> {
    vec![
        home()
            .join(".config/omarchy/backgrounds")
            .join(theme_name()),
        home().join(".local/state/omarchy/current/theme/backgrounds"),
    ]
}

/// Largest monitor (width, height, scale); 4K@1 if hyprctl is unavailable.
pub fn static_target() -> (u32, u32, f32) {
    let parse = || -> Option<(u32, u32, f32)> {
        let out = Command::new("hyprctl")
            .args(["-j", "monitors"])
            .output()
            .ok()?;
        let mons: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        mons.as_array()?
            .iter()
            .filter_map(|m| {
                Some((
                    m["width"].as_u64()? as u32,
                    m["height"].as_u64()? as u32,
                    m["scale"].as_f64()? as f32,
                ))
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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
    let img = snapshot(
        &scene,
        w,
        h,
        font,
        (cfg.font_size * scale).round(),
        &palette,
        seed,
        None,
        gpu,
    )?;
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    let name = format!(
        "current-{}-{}-{}.png",
        now_ms(),
        std::process::id(),
        SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let path = dir.join(&name);
    let previous = std::fs::read_link(background_link()).ok();
    // First install and every theme change (Omarchy links a stock image, then the
    // theme-set hook replaces it) pass through here; remember what `power off` restores.
    if let Some(stock) = previous.as_deref().filter(|p| !p.starts_with(&dir))
        && let Err(e) = record_stock_background(stock)
    {
        eprintln!("asciiwall: cannot record stock background: {e}");
    }
    save_png(&img, &path)?;
    set_background(&path);
    // Keep the previous image too: the shell may still be transitioning from it.
    let keep: Vec<&Path> = std::iter::once(path.as_path())
        .chain(previous.as_deref())
        .collect();
    prune_snapshots(&keep)?;
    write_current_scene(id)?;
    release_memory();
    Ok(())
}

pub fn gallery_theme() -> String {
    std::fs::read_to_string(gallery_dir().join(".theme"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

pub fn gallery_is_stale(cfg: &Config) -> bool {
    let ids = scenes::enabled_ids(&cfg.scenes);
    let dir = gallery_dir();
    gallery_theme() != theme_name()
        || ids
            .iter()
            .any(|id| !dir.join(format!("{id}.png")).is_file())
        || std::fs::read_dir(dir).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                let path = entry.path();
                path.extension().is_some_and(|ext| ext == "png")
                    && path
                        .file_stem()
                        .is_some_and(|stem| !ids.iter().any(|id| id.as_str() == stem))
            })
        })
}

/// Gallery thumbnails: 1080p from a fixed seed at the configured font size.
/// Picker previews start from this exact frame, so a tile does not jump when
/// its loop replaces the still.
pub const GALLERY_WIDTH: u32 = 1920;
pub const GALLERY_HEIGHT: u32 = 1080;
pub const GALLERY_SEED: u64 = 1;

/// Render 1080p thumbnails of every enabled scene for the background picker.
pub fn render_gallery(cfg: &Config, font: &Font, gpu: &GpuCell) -> Result<()> {
    let dir = gallery_dir();
    std::fs::create_dir_all(&dir)?;
    let palette = Palette::load();
    let ids = scenes::enabled_ids(&cfg.scenes);
    for id in &ids {
        let scene = scenes::resolve(id)?;
        let img = snapshot(
            &scene,
            GALLERY_WIDTH,
            GALLERY_HEIGHT,
            font,
            cfg.font_size.round(),
            &palette,
            GALLERY_SEED,
            None,
            gpu,
        )?;
        save_png(&img, &dir.join(format!("{id}.png")))?;
    }
    for e in std::fs::read_dir(&dir)?.flatten() {
        let p = e.path();
        let stale = p.extension().is_some_and(|x| x == "png")
            && p.file_stem()
                .is_some_and(|s| !ids.iter().any(|id| id.as_str() == s));
        if stale {
            let _ = std::fs::remove_file(p);
        }
    }
    std::fs::write(dir.join(".theme"), theme_name())?;
    release_memory();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_background_must_sit_directly_in_a_theme_dir() {
        let root = std::env::temp_dir().join(format!("asciiwall-stock-{}", std::process::id()));
        let theme = root.join("theme/backgrounds");
        let other = root.join("old-theme");
        std::fs::create_dir_all(theme.join("nested")).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        for f in [
            theme.join("a.jpg"),
            theme.join("nested/b.jpg"),
            other.join("c.jpg"),
        ] {
            std::fs::write(f, b"").unwrap();
        }
        let dirs = [root.join("missing"), theme.clone()];
        assert!(inside_any(&theme.join("a.jpg"), &dirs));
        assert!(inside_any(&root.join("theme/./backgrounds/a.jpg"), &dirs));
        assert!(!inside_any(&theme.join("nested/b.jpg"), &dirs));
        assert!(!inside_any(&other.join("c.jpg"), &dirs));
        assert!(!inside_any(&theme.join("gone.jpg"), &dirs));
        std::fs::remove_dir_all(root).unwrap();
    }
}
