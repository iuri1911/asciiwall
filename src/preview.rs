//! Short animated WebP loops of every enabled scene. The `asciiwall.picker` shell
//! plugin plays the selected one; Omarchy's own picker can only show stills.

use crate::config::Config;
use crate::glyphs::Font;
use crate::gpu::GpuCell;
use crate::omarchy::{GALLERY_HEIGHT, GALLERY_SEED, GALLERY_WIDTH, data_dir};
use crate::raster;
use crate::scenes;
use crate::theme::{Palette, theme_name};
use anyhow::{Context, Result, bail};
use image::imageops::{self, FilterType};
use std::fs::File;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The picker's selected tile is 768 logical px wide; 16:9 like the gallery.
const WIDTH: u32 = 768;
const HEIGHT: u32 = 432;
const FPS: u32 = 12;
/// Three seconds: enough to show the motion; the densest scene (`waves`) stays
/// near 1.2 MB.
const FRAMES: u32 = 36;

fn root() -> PathBuf {
    data_dir().join("previews")
}

/// Loops are filed under the theme name that stamps the gallery, so a theme
/// switch never plays old colors. They stay out of the gallery directory, which
/// the stock picker lists when asciiwall is off. A missing or path-like name
/// gets a fixed directory: pruning must never reach `root()` and its lock.
fn theme_dir(theme: &str) -> PathBuf {
    let unusable = theme.is_empty() || theme.starts_with('.') || theme.contains('/');
    root().join(if unusable { "_" } else { theme })
}

/// This theme's loop for each id, `None` where it has not been rendered yet.
pub fn available(ids: &[String]) -> Vec<Option<PathBuf>> {
    let dir = theme_dir(&theme_name());
    ids.iter()
        .map(|id| Some(dir.join(format!("{id}.webp"))).filter(|p| p.is_file()))
        .collect()
}

/// Render this theme's missing loops, or all of them with `all` (an explicit
/// `asciiwall gallery`). Passes never overlap: the background pass that `pick`
/// starts leaves if one is running, an explicit refresh waits for it. A missing
/// or failing ffmpeg is logged once and leaves the picker on still thumbnails.
pub fn render(cfg: &Config, font: &Font, gpu: &GpuCell, all: bool) -> Result<()> {
    std::fs::create_dir_all(root())?;
    let Some(_lock) = lock(all)? else {
        return Ok(());
    };
    let theme = theme_name();
    let dir = theme_dir(&theme);
    std::fs::create_dir_all(&dir)?;
    let ids = scenes::enabled_ids(&cfg.scenes);
    prune(&dir, &ids);
    let palette = Palette::load();
    for id in &ids {
        let out = dir.join(format!("{id}.webp"));
        if !all && out.is_file() {
            continue;
        }
        // The theme changed mid-pass: the next `pick` renders the new colors.
        if theme_name() != theme {
            break;
        }
        if let Err(e) = encode(cfg, font, gpu, &palette, id, &out) {
            eprintln!("asciiwall: animated previews skipped, the picker shows stills: {e:#}");
            break;
        }
    }
    Ok(())
}

/// Start `asciiwall gallery --previews` detached, so `pick` opens at once. Its
/// own process group keeps it alive when the terminal `pick` ran from closes.
pub fn spawn_missing() {
    let spawned = std::env::current_exe().and_then(|exe| {
        Command::new(exe)
            .args(["gallery", "--previews"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .process_group(0)
            .spawn()
    });
    match spawned {
        // Reaped here if it finishes while the picker is still open.
        Ok(mut child) => {
            std::thread::spawn(move || child.wait());
        }
        Err(e) => eprintln!("asciiwall: cannot start preview rendering: {e}"),
    }
}

/// Exclusive `flock` on `previews/.lock`, released when the file closes (also
/// when a pass is killed). `None` if another pass holds it and `wait` is false.
fn lock(wait: bool) -> Result<Option<File>> {
    let path = root().join(".lock");
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .with_context(|| format!("cannot open {}", path.display()))?;
    let op = if wait {
        libc::LOCK_EX
    } else {
        libc::LOCK_EX | libc::LOCK_NB
    };
    // SAFETY: flock on a descriptor owned by `file` for the whole call.
    if unsafe { libc::flock(file.as_raw_fd(), op) } == 0 {
        return Ok(Some(file));
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(None)
    } else {
        Err(err).with_context(|| format!("cannot lock {}", path.display()))
    }
}

/// Drop other themes' loops, loops of scenes no longer enabled, and temp files
/// left by a killed pass.
fn prune(dir: &Path, ids: &[String]) {
    let entries = |d: &Path| std::fs::read_dir(d).into_iter().flatten().flatten();
    for e in entries(&root()) {
        if e.path() != dir && e.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
    for e in entries(dir) {
        let p = e.path();
        let keep = p.extension().is_some_and(|x| x == "webp")
            && p.file_stem()
                .is_some_and(|s| ids.iter().any(|id| id.as_str() == s));
        if !keep {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// Render one scene exactly like its gallery thumbnail, downscale every frame
/// and pipe it through ffmpeg's animated WebP encoder (temp file + rename).
/// Lossless: flat backgrounds and thin glyphs compress about as well as lossy
/// q75, and the theme colors stay exact (lossy 4:2:0 smears them), so the tile
/// does not shift color when the loop replaces the still.
fn encode(
    cfg: &Config,
    font: &Font,
    gpu: &GpuCell,
    palette: &Palette,
    id: &str,
    out: &Path,
) -> Result<()> {
    let scene = scenes::resolve(id)?;
    let tmp = out.with_extension("webp.tmp");
    let (size, fps) = (format!("{WIDTH}x{HEIGHT}"), FPS.to_string());
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error"])
        .args(["-f", "rawvideo", "-pix_fmt", "rgb24"])
        .args(["-s", &size, "-r", &fps, "-i", "-", "-an"])
        .args(["-c:v", "libwebp_anim", "-lossless", "1"])
        .args(["-compression_level", "1", "-loop", "0"])
        .args(["-f", "webp", "-y"])
        .arg(&tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("cannot run ffmpeg")?;
    let mut stdin = ffmpeg.stdin.take().expect("piped stdin");
    let mut written = Ok(());
    let rendered = raster::frames(
        &scene,
        GALLERY_WIDTH,
        GALLERY_HEIGHT,
        font,
        cfg.font_size.round(),
        palette,
        GALLERY_SEED,
        None,
        gpu,
        FRAMES,
        1.0 / FPS as f32,
        |img| {
            if written.is_ok() {
                let small = imageops::resize(&img, WIDTH, HEIGHT, FilterType::Triangle);
                written = stdin.write_all(small.as_raw());
            }
        },
    );
    drop(stdin);
    let status = ffmpeg.wait().context("ffmpeg")?;
    let done = rendered.and_then(|()| {
        written.context("ffmpeg stopped reading frames")?;
        if !status.success() {
            bail!("ffmpeg exited with {status}");
        }
        std::fs::rename(&tmp, out)?;
        Ok(())
    });
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done.with_context(|| format!("preview of {id}"))
}
