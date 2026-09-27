use crate::config::{Config, Mode};
use crate::glyphs::Font;
use crate::gpu::{Gpu, GpuCell};
use crate::omarchy::{self, apply_scene, now_ms};
use crate::{ipc, live, scenes};
use anyhow::Result;
use calloop::signals::{Signal, Signals};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, LoopSignal};
use std::time::Instant;

/// State shared by the static and live daemons.
pub struct Core {
    pub cfg: Config,
    pub font: Font,
    pub current: Option<String>,
    pub last_change: Instant,
    /// Live mode's device, reused by background snapshots so they don't open a second one.
    pub gpu: Option<Gpu>,
}

/// Random enabled scene different from `current` (when possible).
pub fn pick_random(cfg: &Config, current: Option<&str>) -> String {
    let ids = scenes::enabled_ids(&cfg.scenes);
    let pool: Vec<&String> = ids
        .iter()
        .filter(|id| Some(id.as_str()) != current)
        .collect();
    let pool = if pool.is_empty() {
        ids.iter().collect()
    } else {
        pool
    };
    pool[fastrand::usize(0..pool.len())].clone()
}

impl Core {
    /// Requested scene (if valid) or a random one.
    fn next_id(&self) -> String {
        if let Some(req) = ipc::take_request() {
            match scenes::resolve(&req) {
                Ok(_) => return req,
                Err(e) => eprintln!("asciiwall: {e}"),
            }
        }
        pick_random(&self.cfg, self.current.as_deref())
    }

    /// The live device if there is one; otherwise a device opened on first use and
    /// dropped with the cell (static mode holds no GPU between rotations).
    pub fn gpu_cell(&self) -> GpuCell {
        self.gpu
            .clone()
            .map_or_else(GpuCell::default, GpuCell::with)
    }

    /// Regenerate picker thumbnails in the background after a theme change.
    fn refresh_gallery(&self) {
        if !omarchy::gallery_is_stale(&self.cfg) {
            return;
        }
        let (cfg, font, gpu) = (self.cfg.clone(), self.font.clone(), self.gpu.clone());
        std::thread::spawn(move || {
            let gpu = gpu.map_or_else(GpuCell::default, GpuCell::with);
            if let Err(e) = omarchy::render_gallery(&cfg, &font, &gpu) {
                eprintln!("asciiwall: gallery: {e:#}");
            }
        });
    }
}

pub trait Host: 'static {
    fn core(&mut self) -> &mut Core;
    /// Show scene `id` rendered with `seed`.
    fn show(&mut self, id: &str, seed: u64);
    fn loop_signal(&self) -> &LoopSignal;
}

pub fn rotate<H: Host>(host: &mut H, id: String) {
    let seed = now_ms();
    eprintln!("asciiwall: scene {id}");
    host.show(&id, seed);
    let core = host.core();
    core.current = Some(id);
    core.last_change = Instant::now();
    core.refresh_gallery();
}

/// SIGUSR1 → rotate, SIGTERM/SIGINT → stop, and an optional rotation timer.
pub fn install_common<H: Host>(
    handle: &LoopHandle<'static, H>,
    signals: Signals,
    interval: Option<std::time::Duration>,
) -> Result<()> {
    handle
        .insert_source(signals, |ev, _, host: &mut H| match ev.signal() {
            Signal::SIGUSR1 => {
                let id = host.core().next_id();
                rotate(host, id);
            }
            _ => host.loop_signal().stop(),
        })
        .map_err(|e| anyhow::anyhow!("signal source: {e}"))?;
    if let Some(interval) = interval {
        handle
            .insert_source(Timer::from_duration(interval), move |_, _, host: &mut H| {
                if host.core().last_change.elapsed() >= interval {
                    let id = host.core().next_id();
                    rotate(host, id);
                }
                TimeoutAction::ToInstant(host.core().last_change + interval)
            })
            .map_err(|e| anyhow::anyhow!("timer source: {e}"))?;
    }
    Ok(())
}

/// First scene: pending request, else the last scene shown. Keeping it matters most
/// after `power on`, where the stock background is showing and the user expects the
/// ASCII scene they turned off, not a random one.
pub fn initial_id(core: &Core) -> String {
    if let Some(req) = ipc::take_request().filter(|r| scenes::resolve(r).is_ok()) {
        return req;
    }
    match &core.current {
        Some(id) if scenes::resolve(id).is_ok() => id.clone(),
        _ => pick_random(&core.cfg, core.current.as_deref()),
    }
}

struct StaticHost {
    core: Core,
    signal: LoopSignal,
}

impl Host for StaticHost {
    fn core(&mut self) -> &mut Core {
        &mut self.core
    }
    fn show(&mut self, id: &str, seed: u64) {
        if let Err(e) = apply_scene(
            &self.core.cfg,
            &self.core.font,
            id,
            seed,
            &self.core.gpu_cell(),
        ) {
            eprintln!("asciiwall: {e:#}");
        }
    }
    fn loop_signal(&self) -> &LoopSignal {
        &self.signal
    }
}

pub fn run() -> Result<()> {
    let cfg = Config::load()?;
    // Exit 0 so `Restart=on-failure` leaves a turned-off daemon stopped.
    if !cfg.enabled {
        eprintln!("asciiwall: disabled; exiting");
        return Ok(());
    }
    // Block the signals before any thread exists so every thread inherits the mask.
    let signals = Signals::new(&[Signal::SIGUSR1, Signal::SIGTERM, Signal::SIGINT])?;
    ipc::claim_pidfile()?;
    let result = (|| {
        let font = crate::glyphs::load_font(&cfg.font_family)?;
        let core = Core {
            cfg,
            font,
            current: omarchy::current_scene(),
            last_change: Instant::now(),
            gpu: None,
        };
        match core.cfg.mode {
            Mode::Static => run_static(core, signals),
            Mode::Live => live::run(core, signals),
        }
    })();
    ipc::release_pidfile();
    result
}

fn run_static(core: Core, signals: Signals) -> Result<()> {
    let mut event_loop: EventLoop<'static, StaticHost> = EventLoop::try_new()?;
    let interval = core.cfg.interval();
    install_common(&event_loop.handle(), signals, interval)?;
    let mut host = StaticHost {
        core,
        signal: event_loop.get_signal(),
    };
    let id = initial_id(&host.core);
    rotate(&mut host, id);
    event_loop.run(None, &mut host, |_| {})?;
    Ok(())
}
