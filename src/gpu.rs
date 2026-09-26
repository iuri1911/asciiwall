//! Headless wgpu device shared by live rendering and shader scenes.

use anyhow::{Context, Result};
use std::cell::OnceCell;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// Held around `Surface::configure` and around submissions made off the render
    /// thread: wgpu fails a reconfigure if another thread submits while it waits for idle.
    pub serial: Arc<Mutex<()>>,
}

impl Gpu {
    pub fn new() -> Result<Gpu> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapters = pollster::block_on(instance.enumerate_adapters(wgpu::Backends::VULKAN));
        let adapter = match scanout_gpu().and_then(|id| adapters.into_iter().find(|a| pci_id(a) == id)) {
            Some(a) => a,
            None => pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                ..Default::default()
            }))
            .context("no GPU adapter")?,
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).context("no GPU device")?;
        Ok(Gpu { instance, adapter, device, queue, serial: Arc::default() })
    }
}

fn pci_id(a: &wgpu::Adapter) -> (u32, u32) {
    let info = a.get_info();
    (info.vendor, info.device)
}

/// PCI (vendor, device) of the GPU driving the most connected monitors. Rendering
/// anywhere else makes every frame cross GPUs (PRIME copies through system memory),
/// which costs far more than drawing a character grid.
fn scanout_gpu() -> Option<(u32, u32)> {
    let hex = |p: std::path::PathBuf| -> Option<u32> {
        let s = std::fs::read_to_string(p).ok()?;
        u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok()
    };
    let drm = std::path::Path::new("/sys/class/drm");
    let mut counts: std::collections::HashMap<(u32, u32), u32> = std::collections::HashMap::new();
    for entry in std::fs::read_dir(drm).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((card, _connector)) = name.split_once('-') else { continue };
        let connected = std::fs::read_to_string(entry.path().join("status")).is_ok_and(|s| s.trim() == "connected");
        if !connected {
            continue;
        }
        let dev = drm.join(card).join("device");
        if let (Some(v), Some(d)) = (hex(dev.join("vendor")), hex(dev.join("device"))) {
            *counts.entry((v, d)).or_default() += 1;
        }
    }
    counts.into_iter().max_by_key(|&(_, n)| n).map(|(id, _)| id)
}

/// A device created on first use, so CPU-only scenes never initialize Vulkan.
#[derive(Default)]
pub struct GpuCell(OnceCell<Gpu>);

impl GpuCell {
    pub fn with(gpu: Gpu) -> GpuCell {
        GpuCell(OnceCell::from(gpu))
    }

    pub fn get(&self) -> Result<&Gpu> {
        if let Some(g) = self.0.get() {
            return Ok(g);
        }
        let g = Gpu::new()?;
        Ok(self.0.get_or_init(|| g))
    }
}
