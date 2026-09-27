//! Reproducible, theme-colored clips for sharing. Frames come from the same
//! scene path as the picker, but are long enough to show slow animations.

use crate::config::Config;
use crate::glyphs::Font;
use crate::gpu::GpuCell;
use crate::omarchy::GALLERY_SEED;
use crate::raster;
use crate::scenes;
use crate::theme::Palette;
use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;
const FPS: u32 = 24;
const SECONDS: u32 = 10;

pub fn export(
    cfg: &Config,
    font: &Font,
    gpu: &GpuCell,
    dir: &Path,
    selected: Option<&str>,
) -> Result<()> {
    let ids: Vec<String> = scenes::enabled_ids(&cfg.scenes)
        .into_iter()
        .filter(|id| !scenes::is_plate(id))
        .filter(|id| selected.is_none_or(|name| name == id))
        .collect();
    if ids.is_empty() {
        bail!(
            "no enabled animated scene matches {}",
            selected.unwrap_or("the configuration")
        );
    }
    let mp4_dir = dir.join("mp4");
    let gif_dir = dir.join("gif");
    std::fs::create_dir_all(&mp4_dir)?;
    std::fs::create_dir_all(&gif_dir)?;
    let palette = Palette::load();
    for id in ids {
        let video = mp4_dir.join(format!("{id}.mp4"));
        let gif = gif_dir.join(format!("{id}.gif"));
        println!("exporting {id}");
        encode_video(cfg, font, gpu, &palette, &id, &video)?;
        encode_gif(&video, &gif)?;
    }
    Ok(())
}

fn encode_video(
    cfg: &Config,
    font: &Font,
    gpu: &GpuCell,
    palette: &Palette,
    id: &str,
    out: &Path,
) -> Result<()> {
    let scene = scenes::resolve(id)?;
    let tmp = out.with_extension("mp4.tmp");
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
        ])
        .args([
            "-s",
            &format!("{WIDTH}x{HEIGHT}"),
            "-r",
            &FPS.to_string(),
            "-i",
            "-",
            "-an",
        ])
        .args([
            "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p",
        ])
        .args(["-movflags", "+faststart", "-f", "mp4", "-y"])
        .arg(&tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("cannot start ffmpeg for MP4")?;
    let mut stdin = ffmpeg.stdin.take().expect("piped stdin");
    let mut written = Ok(());
    let rendered = raster::frames(
        &scene,
        WIDTH,
        HEIGHT,
        font,
        cfg.font_size.round(),
        palette,
        GALLERY_SEED,
        None,
        gpu,
        FPS * SECONDS,
        1.0 / FPS as f32,
        |img| {
            if written.is_ok() {
                written = stdin.write_all(img.as_raw());
            }
        },
    );
    drop(stdin);
    let status = ffmpeg.wait().context("ffmpeg MP4")?;
    let done = rendered.and_then(|()| {
        written.context("ffmpeg stopped reading MP4 frames")?;
        if !status.success() {
            bail!("ffmpeg MP4 exited with {status}");
        }
        std::fs::rename(&tmp, out)?;
        Ok(())
    });
    if done.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    done.with_context(|| format!("MP4 of {id}"))
}

fn encode_gif(video: &Path, out: &Path) -> Result<()> {
    let tmp = out.with_extension("gif.tmp");
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(video)
        .args([
            "-filter_complex",
            "[0:v]fps=8,scale=480:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=32:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle",
            "-loop", "0", "-f", "gif", "-y",
        ])
        .arg(&tmp)
        .output()
        .context("cannot start ffmpeg for GIF")?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&tmp);
        bail!("ffmpeg GIF: {}", String::from_utf8_lossy(&output.stderr));
    }
    std::fs::rename(&tmp, out)?;
    Ok(())
}
