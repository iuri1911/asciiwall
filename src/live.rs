//! Live mode: one wlr-layer-shell surface per output on the Bottom layer (above
//! omarchy-shell's Background layer, below windows), rendered on the GPU with wgpu.
//! Each frame fills a buffer of packed (glyph, color) cells, either uploaded from a
//! CPU scene or written in place by a shader scene's compute pass; the fragment
//! shader expands it through a glyph-coverage atlas.

use crate::daemon::{Core, Host, initial_id, install_common, rotate};
use crate::glyphs::Atlas;
use crate::gpu::{Gpu, GpuCell};
use crate::grid::{Grid, layout};
use crate::omarchy::apply_scene;
use crate::raster::SNAPSHOT_DT;
use crate::scenes::{self, Scene, SceneId, Setup};
use crate::theme::Palette;
use anyhow::{Context, Result, anyhow};
use calloop::generic::Generic;
use calloop::signals::Signals;
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, Interest, LoopSignal, PostAction};
use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::{delegate_registry, registry_handlers};
use std::collections::HashMap;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::ptr::NonNull;
use std::time::{Duration, Instant};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_output::{Transform, WlOutput};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::viewporter::client::wp_viewport::{self, WpViewport};
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;

const UNIFORM_SIZE: usize = 32 + 8 * 16;
const HYPR_EVENTS: [&[u8]; 6] =
    [b"fullscreen>>", b"workspace>>", b"focusedmon>>", b"openwindow>>", b"closewindow>>", b"movewindow>>"];

/// GPU resources for one configured output.
struct Res {
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    /// Upload target for CPU scenes; shader scenes bring their own buffer.
    cells: wgpu::Buffer,
    atlas_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    atlas: Atlas,
    off: (u32, u32),
    srgb: bool,
    grid: Grid,
    /// Last uploaded CPU grid; an identical frame is not presented again.
    shown: Vec<u8>,
    scene: Box<dyn Scene>,
    t: f32,
}

/// Field order matters: the wgpu surface must drop before the wl_surface (in `layer`).
struct Out {
    res: Option<Res>,
    surface: wgpu::Surface<'static>,
    viewport: Option<WpViewport>,
    layer: LayerSurface,
    output: WlOutput,
    name: String,
    logical: (u32, u32),
    paused: bool,
    /// A frame callback is outstanding: the compositor hasn't shown the last frame yet.
    /// It never answers while the output is off, so nothing renders then.
    waiting: bool,
}

impl Drop for Out {
    fn drop(&mut self) {
        if let Some(v) = &self.viewport {
            v.destroy();
        }
    }
}

struct Render {
    gpu: Gpu,
    /// The same device, handed to shader scenes.
    cell: GpuCell,
    shader: wgpu::ShaderModule,
    bgl: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
}

struct LiveApp {
    core: Core,
    signal: LoopSignal,
    conn: Connection,
    qh: QueueHandle<LiveApp>,
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    viewporter: Option<WpViewporter>,
    render: Render,
    outs: Vec<Out>,
    scene_id: SceneId,
    seed: u64,
    palette: Palette,
    last_frame: Instant,
    hypr_buf: Vec<u8>,
}

pub fn run(mut core: Core, signals: Signals) -> Result<()> {
    let conn = Connection::connect_to_env().context("cannot connect to the Wayland display")?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();
    let mut event_loop: EventLoop<'static, LiveApp> = EventLoop::try_new()?;
    let handle = event_loop.handle();
    WaylandSource::new(conn.clone(), event_queue)
        .insert(handle.clone())
        .map_err(|e| anyhow!("wayland source: {}", e.error))?;

    let compositor = CompositorState::bind(&globals, &qh).context("wl_compositor not available")?;
    let layer_shell = LayerShell::bind(&globals, &qh).context("wlr layer shell not available")?;
    let viewporter = globals.bind::<WpViewporter, _, _>(&qh, 1..=1, ()).ok();

    let render = init_render()?;
    core.gpu = Some(render.gpu.clone());
    let interval = core.cfg.interval();
    let frame = Duration::from_secs_f64(1.0 / core.cfg.fps.clamp(1, 240) as f64);
    install_common(&handle, signals, interval)?;
    handle
        .insert_source(Timer::from_duration(frame), move |deadline, _, app: &mut LiveApp| {
            app.draw();
            TimeoutAction::ToInstant((deadline + frame).max(Instant::now()))
        })
        .map_err(|e| anyhow!("frame timer: {e}"))?;
    if let Err(e) = watch_hyprland(&handle) {
        eprintln!("asciiwall: fullscreen pause disabled: {e:#}");
    }

    let first = initial_id(&core);
    let mut app = LiveApp {
        core,
        signal: event_loop.get_signal(),
        conn,
        qh: qh.clone(),
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor,
        layer_shell,
        viewporter,
        render,
        outs: Vec::new(),
        scene_id: SceneId::Pattern(scenes::PATTERNS[0]),
        seed: 0,
        palette: Palette::load(),
        last_frame: Instant::now(),
        hypr_buf: Vec::new(),
    };
    rotate(&mut app, first);
    app.refresh_pause();
    event_loop.run(None, &mut app, |_| {})?;
    Ok(())
}

fn init_render() -> Result<Render> {
    let gpu = Gpu::new()?;
    let device = &gpu.device;
    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
    let entry = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty,
        count: None,
    };
    let buffer = |ty| wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None };
    let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("asciiwall"),
        entries: &[
            entry(0, buffer(wgpu::BufferBindingType::Uniform)),
            entry(1, buffer(wgpu::BufferBindingType::Storage { read_only: true })),
            entry(
                2,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
            ),
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("asciiwall"),
        bind_group_layouts: &[Some(&bgl)],
        immediate_size: 0,
    });
    Ok(Render { cell: GpuCell::with(gpu.clone()), gpu, shader, bgl, layout })
}

/// Bind the uniform, the cells the scene writes to (its own buffer for shader
/// scenes, the CPU upload buffer otherwise), and the atlas.
fn bind_group(
    render: &Render,
    uniform: &wgpu::Buffer,
    cpu_cells: &wgpu::Buffer,
    scene: &dyn Scene,
    atlas_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    let cells = scene.gpu().map_or(cpu_cells, |s| s.cells());
    render.gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("asciiwall"),
        layout: &render.bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: uniform.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: cells.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(atlas_view) },
        ],
    })
}

/// Pause outputs whose active workspace has a fullscreen window.
fn watch_hyprland(handle: &calloop::LoopHandle<'static, LiveApp>) -> Result<()> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").context("not running under Hyprland")?;
    let runtime = std::env::var("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR unset")?;
    let path = format!("{runtime}/hypr/{sig}/.socket2.sock");
    let stream = UnixStream::connect(&path).with_context(|| format!("cannot connect {path}"))?;
    stream.set_nonblocking(true)?;
    handle
        .insert_source(Generic::new(stream, Interest::READ, calloop::Mode::Level), |_, stream, app: &mut LiveApp| {
            let mut buf = [0u8; 4096];
            loop {
                match (&**stream).read(&mut buf) {
                    Ok(0) => return Ok(PostAction::Remove),
                    Ok(n) => app.hypr_buf.extend_from_slice(&buf[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                }
            }
            let mut relevant = false;
            while let Some(pos) = app.hypr_buf.iter().position(|&b| b == b'\n') {
                relevant |= HYPR_EVENTS.iter().any(|e| app.hypr_buf.starts_with(e));
                app.hypr_buf.drain(..=pos);
            }
            if relevant {
                app.refresh_pause();
            }
            Ok(PostAction::Continue)
        })
        .map_err(|e| anyhow!("hyprland socket: {e}"))?;
    Ok(())
}

/// Monitor name → active workspace has a fullscreen window.
fn fullscreen_monitors() -> HashMap<String, bool> {
    let json = |what: &str| -> Option<serde_json::Value> {
        let out = std::process::Command::new("hyprctl").args(["-j", what]).output().ok()?;
        serde_json::from_slice(&out.stdout).ok()
    };
    let (Some(mons), Some(wss)) = (json("monitors"), json("workspaces")) else {
        return HashMap::new();
    };
    let full: HashMap<i64, bool> = wss
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|w| Some((w["id"].as_i64()?, w["hasfullscreen"].as_bool()?)))
        .collect();
    mons.as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let ws = m["activeWorkspace"]["id"].as_i64()?;
            Some((m["name"].as_str()?.to_string(), full.get(&ws).copied().unwrap_or(false)))
        })
        .collect()
}

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn uniform_bytes(res: &Res, palette: &Palette) -> [u8; UNIFORM_SIZE] {
    let mut b = [0u8; UNIFORM_SIZE];
    let words = [res.atlas.cell_w, res.atlas.cell_h, res.grid.cols, res.grid.rows, res.off.0, res.off.1, 0, 0];
    for (i, w) in words.iter().enumerate() {
        b[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    for (slot, rgb) in palette.colors.iter().enumerate() {
        for c in 0..4 {
            let mut v = if c == 3 { 1.0 } else { rgb[c] as f32 / 255.0 };
            if res.srgb && c < 3 {
                v = to_linear(v);
            }
            let at = 32 + slot * 16 + c * 4;
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    b
}

impl LiveApp {
    fn add_output(&mut self, qh: &QueueHandle<Self>, output: WlOutput) {
        let name = self.output_state.info(&output).and_then(|i| i.name).unwrap_or_default();
        let wl_surface = self.compositor.create_surface(qh);
        let layer =
            self.layer_shell.create_layer_surface(qh, wl_surface, Layer::Bottom, Some("asciiwall"), Some(&output));
        layer.set_anchor(Anchor::all());
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        match Region::new(&self.compositor) {
            Ok(region) => layer.set_input_region(Some(region.wl_region())),
            Err(e) => eprintln!("asciiwall: cannot create input region: {e}"),
        }
        let viewport = self.viewporter.as_ref().map(|vp| vp.get_viewport(layer.wl_surface(), qh, ()));
        layer.commit();

        let display = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(self.conn.backend().display_ptr() as *mut _).expect("null wl_display"),
        ));
        let window = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(layer.wl_surface().id().as_ptr() as *mut _).expect("null wl_surface"),
        ));
        // SAFETY: the display outlives the app; `Out` drops the wgpu surface before the wl_surface.
        let surface = match unsafe {
            self.render.gpu.instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(display),
                raw_window_handle: window,
            })
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("asciiwall: cannot create GPU surface for {name}: {e}");
                self.signal.stop();
                return;
            }
        };
        self.outs.push(Out {
            res: None,
            surface,
            viewport,
            layer,
            output,
            name,
            logical: (0, 0),
            paused: false,
            waiting: false,
        });
    }

    /// Physical pixel size of an output's current mode (rotation-aware).
    fn physical_size(&self, output: &WlOutput) -> Option<(u32, u32)> {
        let info = self.output_state.info(output)?;
        let (w, h) = info.modes.iter().find(|m| m.current)?.dimensions;
        Some(match info.transform {
            Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270 => (h as u32, w as u32),
            _ => (w as u32, h as u32),
        })
    }

    fn setup(&mut self, i: usize) -> Result<()> {
        let (lw, lh) = self.outs[i].logical;
        if lw == 0 || lh == 0 {
            return Ok(());
        }
        let (pw, ph) = if self.outs[i].viewport.is_some() {
            let (pw, ph) = self.physical_size(&self.outs[i].output).unwrap_or((lw, lh));
            self.outs[i].viewport.as_ref().unwrap().set_destination(lw as i32, lh as i32);
            (pw, ph)
        } else {
            let s = self.output_state.info(&self.outs[i].output).map_or(1, |i| i.scale_factor.max(1)) as u32;
            let _ = self.outs[i].layer.set_buffer_scale(s);
            (lw * s, lh * s)
        };
        if let Some(res) = &self.outs[i].res
            && (res.config.width, res.config.height) == (pw, ph)
        {
            return Ok(());
        }
        self.outs[i].res = None;
        self.outs[i].waiting = false;

        let render = &self.render;
        let gpu = &render.gpu;
        let out = &self.outs[i];
        let caps = out.surface.get_capabilities(&gpu.adapter);
        // 8-bit UNORM keeps shader blending in sRGB byte space, identical to the PNG snapshots.
        // Float formats are scRGB-linear, so fall back to an sRGB format (palette linearized) instead.
        let format = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
            .into_iter()
            .find(|f| caps.formats.contains(f))
            .or_else(|| caps.formats.iter().copied().find(|f| f.is_srgb()))
            .context("surface has no 8-bit formats")?;
        let mut config = out.surface.get_default_config(&gpu.adapter, pw, ph).context("surface unsupported")?;
        config.format = format;
        config.view_formats = vec![];
        config.present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        config.alpha_mode = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        configure(gpu, &out.surface, &config);

        let scale = pw as f32 / lw as f32;
        let atlas = Atlas::new(&self.core.font, (self.core.cfg.font_size * scale).round());
        let (cols, rows, ox, oy) = layout(pw, ph, atlas.cell_w, atlas.cell_h);

        let atlas_tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas"),
            size: wgpu::Extent3d { width: atlas.width(), height: atlas.cell_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            atlas_tex.as_image_copy(),
            &atlas.coverage,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(atlas.width()), rows_per_image: None },
            atlas_tex.size(),
        );
        let atlas_view = atlas_tex.create_view(&Default::default());
        let cells = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cells"),
            size: (cols * rows).div_ceil(2) as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform"),
            size: UNIFORM_SIZE as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("asciiwall"),
            layout: Some(&render.layout),
            vertex: wgpu::VertexState {
                module: &render.shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &render.shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let aspect = (cols * atlas.cell_w) as f32 / (rows * atlas.cell_h) as f32;
        let setup = Setup {
            cols,
            rows,
            aspect,
            seed: self.seed,
            palette: &self.palette,
            atlas: &atlas,
            gpu: &render.cell,
        };
        let scene = scenes::build(&self.scene_id, &setup)?;
        let bind_group = bind_group(render, &uniform, &cells, scene.as_ref(), &atlas_view);
        let mut res = Res {
            config,
            pipeline,
            cells,
            atlas_view,
            uniform,
            bind_group,
            atlas,
            off: (ox, oy),
            srgb: format.is_srgb(),
            grid: Grid::new(cols, rows),
            shown: Vec::new(),
            scene,
            t: 0.0,
        };
        warmup(&mut res, self.seed);
        gpu.queue.write_buffer(&res.uniform, 0, &uniform_bytes(&res, &self.palette));
        eprintln!(
            "asciiwall: {} {}x{} ({}x{} logical) {:?} {:?}, grid {}x{}",
            self.outs[i].name, pw, ph, lw, lh, format, res.config.present_mode, cols, rows
        );
        self.outs[i].res = Some(res);
        Ok(())
    }

    /// Swap in the current scene/palette on every configured output.
    fn rebuild_scenes(&mut self) {
        let render = &self.render;
        for out in &mut self.outs {
            let Some(res) = &mut out.res else { continue };
            let setup = Setup {
                cols: res.grid.cols,
                rows: res.grid.rows,
                aspect: (res.grid.cols * res.atlas.cell_w) as f32 / (res.grid.rows * res.atlas.cell_h) as f32,
                seed: self.seed,
                palette: &self.palette,
                atlas: &res.atlas,
                gpu: &render.cell,
            };
            match scenes::build(&self.scene_id, &setup) {
                Ok(scene) => {
                    res.bind_group = bind_group(render, &res.uniform, &res.cells, scene.as_ref(), &res.atlas_view);
                    res.scene = scene;
                    res.shown.clear();
                    warmup(res, self.seed);
                    render.gpu.queue.write_buffer(&res.uniform, 0, &uniform_bytes(res, &self.palette));
                }
                Err(e) => eprintln!("asciiwall: {e:#}"),
            }
        }
    }

    fn draw(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let (gpu, qh) = (&self.render.gpu, &self.qh);
        for out in &mut self.outs {
            if out.paused || out.waiting {
                continue;
            }
            let Some(res) = &mut out.res else { continue };
            res.t += dt;
            let mut enc = gpu.device.create_command_encoder(&Default::default());
            let cpu = res.scene.gpu().is_none();
            if let Some(scene) = res.scene.gpu() {
                scene.encode(&mut enc, res.t);
            } else {
                res.scene.update(res.t, dt, &mut res.grid);
                if res.grid.bytes() == res.shown.as_slice() {
                    continue;
                }
                upload(&gpu.queue, &res.cells, res.grid.bytes());
            }
            let frame = match out.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                    configure(gpu, &out.surface, &res.config);
                    continue;
                }
                _ => continue,
            };
            let view = frame.texture.create_view(&Default::default());
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&res.pipeline);
                pass.set_bind_group(0, &res.bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            gpu.queue.submit([enc.finish()]);
            let surface = out.layer.wl_surface();
            surface.frame(qh, FrameCallbackData(surface.clone()));
            out.waiting = true;
            gpu.queue.present(frame);
            if cpu {
                res.shown.clear();
                res.shown.extend_from_slice(res.grid.bytes());
            }
        }
    }

    fn refresh_pause(&mut self) {
        let full = fullscreen_monitors();
        for out in &mut self.outs {
            let paused = full.get(&out.name).copied().unwrap_or(false);
            if paused != out.paused {
                eprintln!("asciiwall: {} {}", out.name, if paused { "paused" } else { "resumed" });
            }
            out.paused = paused;
        }
    }
}

/// Start a scene where the matching static snapshot starts, then advance it past warm-up.
fn warmup(res: &mut Res, seed: u64) {
    res.t = (seed % 1000) as f32;
    for _ in 0..res.scene.warmup_steps() {
        res.scene.update(res.t, SNAPSHOT_DT, &mut res.grid);
        res.t += SNAPSHOT_DT;
    }
}

fn configure(gpu: &Gpu, surface: &wgpu::Surface, config: &wgpu::SurfaceConfiguration) {
    let _serial = gpu.serial.lock().unwrap_or_else(|e| e.into_inner());
    surface.configure(&gpu.device, config);
}

/// Copy grid bytes into a cell buffer; buffer writes need a 4-byte multiple.
fn upload(queue: &wgpu::Queue, cells: &wgpu::Buffer, bytes: &[u8]) {
    let n = bytes.len() & !3;
    if n > 0 {
        queue.write_buffer(cells, 0, &bytes[..n]);
    }
    if n < bytes.len() {
        queue.write_buffer(cells, n as u64, &[bytes[n], bytes[n + 1], 0, 0]);
    }
}

impl Host for LiveApp {
    fn core(&mut self) -> &mut Core {
        &mut self.core
    }

    fn show(&mut self, id: &str, seed: u64) {
        match scenes::resolve(id) {
            Ok(sid) => {
                self.scene_id = sid;
                self.seed = seed;
                self.palette = Palette::load();
                self.rebuild_scenes();
            }
            Err(e) => eprintln!("asciiwall: {e:#}"),
        }
        // Static snapshot for the lock screen, picker and theme transitions.
        let (cfg, font, id) = (self.core.cfg.clone(), self.core.font.clone(), id.to_string());
        let gpu = self.render.gpu.clone();
        std::thread::spawn(move || {
            if let Err(e) = apply_scene(&cfg, &font, &id, seed, &GpuCell::with(gpu)) {
                eprintln!("asciiwall: {e:#}");
            }
        });
    }

    fn loop_signal(&self) -> &LoopSignal {
        &self.signal
    }
}

impl CompositorHandler for LiveApp {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &WlSurface, _: u32) {
        if let Some(out) = self.outs.iter_mut().find(|o| o.layer.wl_surface() == surface) {
            out.waiting = false;
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
}

impl OutputHandler for LiveApp {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: WlOutput) {
        self.add_output(qh, output);
        self.refresh_pause();
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        if let Some(i) = self.outs.iter().position(|o| o.output == output) {
            if let Some(name) = self.output_state.info(&output).and_then(|i| i.name) {
                self.outs[i].name = name;
            }
            if let Err(e) = self.setup(i) {
                eprintln!("asciiwall: {e:#}");
            }
        }
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        self.outs.retain(|o| o.output != output);
    }
}

impl LayerShellHandler for LiveApp {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        self.outs.retain(|o| &o.layer != layer);
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        let Some(i) = self.outs.iter().position(|o| &o.layer == layer) else { return };
        self.outs[i].logical = configure.new_size;
        if let Err(e) = self.setup(i) {
            eprintln!("asciiwall: {e:#}");
        }
    }
}

impl Dispatch<WpViewport, ()> for LiveApp {
    fn event(_: &mut Self, _: &WpViewport, _: wp_viewport::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

wayland_client::delegate_noop!(LiveApp: WpViewporter);

delegate_registry!(LiveApp);

impl ProvidesRegistryState for LiveApp {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers!(OutputState);
}

smithay_client_toolkit::delegate_dispatch2!(LiveApp);
