mod config;
mod daemon;
mod glyphs;
mod gpu;
mod grid;
mod ipc;
mod live;
mod omarchy;
mod raster;
mod scenes;
mod theme;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use config::{Config, Mode};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "asciiwall", version, about = "Procedural and image ASCII-art wallpapers for Omarchy")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List every scene id
    List,
    /// Render one frame of a scene to a PNG
    Render {
        scene: String,
        out: PathBuf,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        font_px: Option<f32>,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 0.0)]
        time: f32,
    },
    /// Switch to a random scene now
    Next,
    /// Switch to a scene id (or a gallery PNG path)
    Set { scene: String },
    /// Render picker thumbnails for every enabled scene
    Gallery,
    /// Open the Omarchy image picker with the ASCII gallery
    Pick,
    /// Choose live (animated) or static wallpapers
    Mode { mode: ModeArg },
    /// Rotation interval in minutes (0 disables automatic rotation)
    Interval { minutes: u64 },
    /// Print one setting for menu checkmarks
    Status { field: StatusField },
    /// Run the wallpaper daemon
    Daemon,
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    Live,
    Static,
    Toggle,
}

#[derive(Clone, Copy, ValueEnum)]
enum StatusField {
    Mode,
    Interval,
    Scene,
}

fn main() {
    if let Err(e) = run(Cli::parse().cmd) {
        eprintln!("asciiwall: {e:#}");
        std::process::exit(1);
    }
}

fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::List => {
            for id in scenes::all_ids() {
                println!("{id}");
            }
        }
        Cmd::Render { scene, out, width, height, font_px, seed, time } => {
            let cfg = Config::load()?;
            let id = scenes::resolve(&scene)?;
            let (tw, th, scale) = omarchy::static_target();
            let font = glyphs::load_font(&cfg.font_family)?;
            let img = raster::snapshot(
                &id,
                width.unwrap_or(tw),
                height.unwrap_or(th),
                &font,
                font_px.unwrap_or((cfg.font_size * scale).round()),
                &theme::Palette::load(),
                seed,
                Some(time),
                &gpu::GpuCell::default(),
            )?;
            raster::save_png(&img, &out)?;
        }
        Cmd::Next => match ipc::running_pid() {
            Some(pid) => ipc::signal_daemon(pid)?,
            None => {
                let cfg = Config::load()?;
                let id = daemon::pick_random(&cfg, omarchy::current_scene().as_deref());
                apply(&cfg, &id)?;
            }
        },
        Cmd::Set { scene } => set(&scene)?,
        Cmd::Gallery => {
            let cfg = Config::load()?;
            omarchy::render_gallery(&cfg, &glyphs::load_font(&cfg.font_family)?, &gpu::GpuCell::default())?;
        }
        Cmd::Pick => pick()?,
        Cmd::Mode { mode } => {
            let mut cfg = Config::load()?;
            let chosen = match (mode, cfg.mode) {
                (ModeArg::Live, _) | (ModeArg::Toggle, Mode::Static) => Mode::Live,
                (ModeArg::Static, _) | (ModeArg::Toggle, Mode::Live) => Mode::Static,
            };
            if cfg.mode != chosen {
                cfg.mode = chosen;
                cfg.save()?;
                restart_service();
            }
            println!("mode: {}", if cfg.mode == Mode::Live { "live" } else { "static" });
        }
        Cmd::Interval { minutes } => {
            let mut cfg = Config::load()?;
            if cfg.interval_minutes != minutes {
                cfg.interval_minutes = minutes;
                cfg.save()?;
                restart_service();
            }
            println!("interval_minutes: {minutes}");
        }
        Cmd::Status { field } => match field {
            StatusField::Scene => println!("{}", omarchy::current_scene().unwrap_or_default()),
            StatusField::Mode => println!("{}", if Config::load()?.mode == Mode::Live { "live" } else { "static" }),
            StatusField::Interval => println!("{}", Config::load()?.interval_minutes),
        },
        Cmd::Daemon => daemon::run()?,
    }
    Ok(())
}

fn restart_service() {
    let ok = std::process::Command::new("systemctl")
        .args(["--user", "restart", "asciiwall.service"])
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!("asciiwall: note: asciiwall.service not restarted; run `asciiwall daemon` manually");
    }
}

fn apply(cfg: &Config, id: &str) -> Result<()> {
    let font = glyphs::load_font(&cfg.font_family)?;
    omarchy::apply_scene(cfg, &font, id, omarchy::now_ms(), &gpu::GpuCell::default())
}

fn set(target: &str) -> Result<()> {
    let id = if target.ends_with(".png") || target.contains('/') {
        match Path::new(target).file_stem() {
            Some(s) => s.to_string_lossy().into_owned(),
            None => bail!("unknown scene: {target}"),
        }
    } else {
        target.to_string()
    };
    scenes::resolve(&id)?;
    match ipc::running_pid() {
        Some(pid) => {
            ipc::write_request(&id)?;
            ipc::signal_daemon(pid)
        }
        None => apply(&Config::load()?, &id),
    }
}

fn pick() -> Result<()> {
    let cfg = Config::load()?;
    if omarchy::gallery_is_stale(&cfg) {
        omarchy::render_gallery(&cfg, &glyphs::load_font(&cfg.font_family)?, &gpu::GpuCell::default())?;
    }
    let dir = omarchy::gallery_dir();
    let mut cmd = std::process::Command::new("omarchy-menu-images");
    if let Some(cur) = omarchy::current_scene().map(|id| dir.join(format!("{id}.png"))).filter(|p| p.is_file()) {
        cmd.arg("--selected").arg(cur);
    }
    let out = cmd.arg(&dir).output()?;
    let choice = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if choice.is_empty() {
        return Ok(());
    }
    set(&choice)
}
