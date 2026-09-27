mod config;
mod daemon;
mod glyphs;
mod gpu;
mod grid;
mod ipc;
mod live;
mod media;
mod omarchy;
mod preview;
mod raster;
mod scenes;
mod theme;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use config::{Config, Mode};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};

#[derive(Parser)]
#[command(
    name = "asciiwall",
    version,
    about = "Procedural and image ASCII-art wallpapers for Omarchy"
)]
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
    /// Render picker thumbnails and animated previews for every enabled scene
    Gallery {
        /// Only render missing animated previews (what `pick` starts in the background)
        #[arg(long)]
        previews: bool,
    },
    /// Export ten-second MP4 clips and GIFs of enabled animated scenes
    Media {
        #[arg(long, default_value = "media")]
        out_dir: PathBuf,
        #[arg(long)]
        scene: Option<String>,
    },
    /// Open the scene picker over the ASCII gallery
    Pick,
    /// Turn ASCII wallpapers on, or off to get the normal Omarchy background back
    Power { state: PowerArg },
    /// Choose live (animated) or static wallpapers
    Mode { mode: ModeArg },
    /// Rotation interval in minutes (0 disables automatic rotation)
    Interval { minutes: u64 },
    /// Print one setting for menu checkmarks
    Status { field: StatusField },
    /// Pace of one animated scene: low, medium, or high
    Tempo {
        /// Omit to print the current scene's tempo
        level: Option<TempoArg>,
        /// Scene id; defaults to the scene on screen
        #[arg(long)]
        scene: Option<String>,
    },
    /// Validate built-in shaders, or one pack directory
    Check { path: Option<PathBuf> },
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
enum PowerArg {
    On,
    Off,
    Toggle,
}

#[derive(Clone, Copy, ValueEnum)]
enum StatusField {
    Enabled,
    Mode,
    Interval,
    Scene,
    Tempo,
}

#[derive(Clone, Copy, ValueEnum)]
enum TempoArg {
    Low,
    Medium,
    High,
}

impl TempoArg {
    fn tempo(self) -> config::Tempo {
        match self {
            TempoArg::Low => config::Tempo::Low,
            TempoArg::Medium => config::Tempo::Medium,
            TempoArg::High => config::Tempo::High,
        }
    }
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
        Cmd::Render {
            scene,
            out,
            width,
            height,
            font_px,
            seed,
            time,
        } => {
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
        Cmd::Next => {
            let cfg = Config::load()?;
            // The theme-set hook runs this too; when off, the stock image Omarchy just
            // picked for the new theme stays.
            if !cfg.enabled {
                eprintln!("asciiwall: off; keeping the current background");
                return Ok(());
            }
            match ipc::running_pid() {
                Some(pid) => ipc::signal_daemon(pid)?,
                None => {
                    let id = daemon::pick_random(&cfg, omarchy::current_scene().as_deref());
                    apply(&cfg, &id)?;
                }
            }
        }
        Cmd::Set { scene } => set(&scene)?,
        Cmd::Gallery { previews } => {
            let cfg = Config::load()?;
            let font = glyphs::load_font(&cfg.font_family)?;
            let gpu = gpu::GpuCell::default();
            if !previews {
                omarchy::render_gallery(&cfg, &font, &gpu)?;
            }
            preview::render(&cfg, &font, &gpu, !previews)?;
        }
        Cmd::Media { out_dir, scene } => {
            let cfg = Config::load()?;
            let font = glyphs::load_font(&cfg.font_family)?;
            media::export(
                &cfg,
                &font,
                &gpu::GpuCell::default(),
                &out_dir,
                scene.as_deref(),
            )?;
        }
        Cmd::Pick => pick()?,
        Cmd::Power { state } => power(state)?,
        Cmd::Mode { mode } => {
            let mut cfg = Config::load()?;
            let chosen = match (mode, cfg.mode) {
                (ModeArg::Live, _) | (ModeArg::Toggle, Mode::Static) => Mode::Live,
                (ModeArg::Static, _) | (ModeArg::Toggle, Mode::Live) => Mode::Static,
            };
            if cfg.mode != chosen {
                cfg.mode = chosen;
                cfg.save()?;
                restart_service(&cfg);
            }
            println!(
                "mode: {}",
                if cfg.mode == Mode::Live {
                    "live"
                } else {
                    "static"
                }
            );
        }
        Cmd::Interval { minutes } => {
            let mut cfg = Config::load()?;
            if cfg.interval_minutes != minutes {
                cfg.interval_minutes = minutes;
                cfg.save()?;
                restart_service(&cfg);
            }
            println!("interval_minutes: {minutes}");
        }
        Cmd::Status { field } => match field {
            StatusField::Enabled => {
                println!("{}", if Config::load()?.enabled { "on" } else { "off" })
            }
            StatusField::Scene => println!("{}", omarchy::current_scene().unwrap_or_default()),
            StatusField::Mode => println!(
                "{}",
                if Config::load()?.mode == Mode::Live {
                    "live"
                } else {
                    "static"
                }
            ),
            StatusField::Interval => println!("{}", Config::load()?.interval_minutes),
            StatusField::Tempo => {
                let scene = omarchy::current_scene().unwrap_or_default();
                println!("{}", Config::load()?.tempo_of(&scene).as_str());
            }
        },
        Cmd::Tempo { level, scene } => set_tempo(level, scene)?,
        Cmd::Check { path } => scenes::check(path.as_deref())?,
        Cmd::Daemon => daemon::run()?,
    }
    Ok(())
}

/// Run `systemctl --user <args> asciiwall.service`; a missing unit only earns a note.
fn systemctl(args: &[&str], hint: &str) {
    let ok = std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .arg("asciiwall.service")
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        eprintln!(
            "asciiwall: note: `systemctl --user {} asciiwall.service` failed{hint}",
            args.join(" ")
        );
    }
}

/// Apply a saved setting. While off the service stays down; `power on` reads it.
fn restart_service(cfg: &Config) {
    if cfg.enabled {
        systemctl(&["restart"], "; run `asciiwall daemon` manually");
    }
}

fn power(state: PowerArg) -> Result<()> {
    let mut cfg = Config::load()?;
    let on = match state {
        PowerArg::On => true,
        PowerArg::Off => false,
        PowerArg::Toggle => !cfg.enabled,
    };
    if cfg.enabled != on {
        cfg.enabled = on;
        cfg.save()?;
    }
    if on {
        // The starting daemon brings back the last scene, or a pending `set` request.
        systemctl(&["enable", "--now"], "; run `asciiwall daemon` manually");
    } else {
        systemctl(&["disable", "--now"], "");
        // A daemon started by hand is not the unit's. Wait for it to exit so it cannot
        // publish a snapshot after the stock background is back.
        if let Some(pid) = ipc::running_pid() {
            ipc::stop_daemon(pid)?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while ipc::running_pid().is_some() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
        omarchy::restore_stock_background();
    }
    println!("asciiwall: {}", if on { "on" } else { "off" });
    Ok(())
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
    let cfg = Config::load()?;
    if !cfg.enabled {
        // Queue the scene for the daemon that `power on` starts.
        ipc::write_request(&id)?;
        return power(PowerArg::On);
    }
    match ipc::running_pid() {
        Some(pid) => {
            ipc::write_request(&id)?;
            ipc::signal_daemon(pid)
        }
        None => apply(&cfg, &id),
    }
}

fn set_tempo(level: Option<TempoArg>, scene: Option<String>) -> Result<()> {
    let id = match scene {
        Some(id) => id,
        None => omarchy::current_scene()
            .ok_or_else(|| anyhow::anyhow!("no current scene; pass --scene <id>"))?,
    };
    scenes::resolve(&id)?;
    let mut cfg = Config::load()?;
    let Some(level) = level else {
        println!("{id} {}", cfg.tempo_of(&id).as_str());
        return Ok(());
    };
    cfg.set_tempo(&id, level.tempo());
    cfg.save()?;
    println!("{id} {}", level.tempo().as_str());
    if ipc::running_pid().is_some() {
        println!("live pace updates within a second; the picture does not jump");
    }
    Ok(())
}

fn pick() -> Result<()> {
    let cfg = Config::load()?;
    if !cfg.enabled {
        return stock_pick();
    }
    if omarchy::gallery_is_stale(&cfg) {
        omarchy::render_gallery(
            &cfg,
            &glyphs::load_font(&cfg.font_family)?,
            &gpu::GpuCell::default(),
        )?;
    }
    let dir = omarchy::gallery_dir();
    let selected = omarchy::current_scene()
        .map(|id| dir.join(format!("{id}.png")))
        .filter(|p| p.is_file());
    let ids = scenes::enabled_ids(&cfg.scenes);
    let choice = match summon_picker(&dir, &ids, selected.as_deref()) {
        Some(choice) => choice,
        None => image_picker(&dir, selected.as_deref())?,
    };
    if choice.is_empty() {
        return Ok(());
    }
    set(&choice)
}

/// Omarchy's own `style.background` action, for when asciiwall is off.
fn stock_pick() -> Result<()> {
    let out = std::process::Command::new("omarchy-theme-bg-switcher")
        .stderr(std::process::Stdio::inherit())
        .output()?;
    let choice = String::from_utf8_lossy(&out.stdout);
    let choice = choice.trim_end_matches('\n');
    if !choice.is_empty() {
        omarchy::set_background(Path::new(choice));
    }
    Ok(())
}

/// Omarchy's image picker over the gallery stills.
fn image_picker(dir: &Path, selected: Option<&Path>) -> Result<String> {
    let mut cmd = std::process::Command::new("omarchy-menu-images");
    if let Some(cur) = selected {
        cmd.arg("--selected").arg(cur);
    }
    let out = cmd.arg(dir).output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

const PICKER_PLUGIN: &str = "asciiwall.picker";

/// Signal that stopped `pick` while it waited on the overlay (0: none).
static PICK_SIGNAL: AtomicI32 = AtomicI32::new(0);

extern "C" fn record_pick_signal(sig: libc::c_int) {
    PICK_SIGNAL.store(sig, Ordering::Relaxed);
}

/// Record SIGINT/SIGTERM/SIGHUP instead of dying while the overlay holds our
/// request: a killed `pick` (`timeout`, Ctrl+C, a closed terminal) would leave
/// it open, holding the keyboard, with nobody to act on the choice.
fn trap_pick_signals(trap: bool) {
    let handler = if trap {
        record_pick_signal as extern "C" fn(libc::c_int) as libc::sighandler_t
    } else {
        libc::SIG_DFL
    };
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: the handler only stores into an atomic.
        unsafe { libc::signal(sig, handler) };
    }
}

/// Exit on a recorded signal: close our overlay request and remove its files.
fn exit_if_signalled(selection: &Path, done: &Path) {
    let sig = PICK_SIGNAL.load(Ordering::Relaxed);
    if sig == 0 {
        return;
    }
    shell_ipc(&["hide", PICKER_PLUGIN]);
    // The overlay answers a close by creating the done file; let it land first.
    for _ in 0..50 {
        if done.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let _ = std::fs::remove_file(selection);
    let _ = std::fs::remove_file(done);
    std::process::exit(128 + sig);
}

/// Remove selection/done files of `pick` runs that were killed outright.
fn remove_stale_pick_files(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name();
        let pid = name
            .to_str()
            .and_then(|n| n.strip_prefix("asciiwall-pick-"))
            .and_then(|n| n.split('.').next())
            .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        if pid.is_some_and(|p| !Path::new("/proc").join(p).exists()) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Run an `omarchy-shell shell` IPC call; its trimmed answer, empty if it failed.
fn shell_ipc(args: &[&str]) -> String {
    std::process::Command::new("omarchy-shell")
        .arg("shell")
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Open the `asciiwall.picker` shell plugin, which animates the selected scene,
/// and start rendering missing loops in the background. `None` when the shell
/// does not take the request (plugin missing or disabled, shell down, overlay
/// failed to load), so the caller falls back to the still picker. Same file
/// round-trip as `omarchy-menu-images`: the plugin writes the chosen path to the
/// selection file, then creates the done file (also on cancel).
fn summon_picker(dir: &Path, ids: &[String], selected: Option<&Path>) -> Option<String> {
    let run_dir = ipc::runtime_dir();
    remove_stale_pick_files(&run_dir);
    let previews = preview::available(ids);
    let rows: Vec<String> = ids
        .iter()
        .zip(&previews)
        .map(|(id, anim)| (dir.join(format!("{id}.png")), anim))
        .filter(|(png, _)| png.is_file())
        .map(|(png, anim)| {
            let anim = anim.as_ref().map(|a| a.display().to_string());
            format!("{0}\t{0}\t{1}", png.display(), anim.unwrap_or_default())
        })
        .collect();
    let base = run_dir.join(format!("asciiwall-pick-{}", std::process::id()));
    let selection = base.with_extension("selection");
    let done = base.with_extension("done");
    let _ = std::fs::remove_file(&done);
    let payload = serde_json::json!({
        "imageRows": rows.join("\n"),
        "selectedImage": selected.map(|p| p.display().to_string()).unwrap_or_default(),
        "selectionFile": selection.display().to_string(),
        "doneFile": done.display().to_string(),
        "showLabels": true,
        "filterable": true,
    });
    trap_pick_signals(true);
    let missing = previews.iter().any(Option::is_none);
    let choice = await_picker(&payload.to_string(), &selection, &done, missing);
    trap_pick_signals(false);
    exit_if_signalled(&selection, &done);
    choice
}

/// Summon the overlay with `payload` and wait for its answer (see `summon_picker`).
fn await_picker(payload: &str, selection: &Path, done: &Path, missing: bool) -> Option<String> {
    if shell_ipc(&["summon", PICKER_PLUGIN, payload]) != "ok" {
        return None;
    }
    // "ok" only means the plugin is enabled. Its overlay echoes the request's
    // done file once it holds it; one that never loads must not leave us
    // waiting for a done file nobody writes.
    let done_str = done.display().to_string();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !done.exists() && shell_ipc(&["call", PICKER_PLUGIN, "activeDoneFile", ""]) != done_str {
        exit_if_signalled(selection, done);
        if std::time::Instant::now() > deadline {
            eprintln!("asciiwall: {PICKER_PLUGIN} did not open; using the still picker");
            shell_ipc(&["hide", PICKER_PLUGIN]);
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if missing {
        preview::spawn_missing();
    }
    while !done.exists() {
        exit_if_signalled(selection, done);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let choice = std::fs::read_to_string(selection).unwrap_or_default();
    let _ = std::fs::remove_file(selection);
    let _ = std::fs::remove_file(done);
    Some(choice.trim().to_string())
}
