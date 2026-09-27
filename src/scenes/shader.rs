//! Scenes evaluated per character cell in a compute shader. The GPU writes packed
//! (glyph, slot) cells straight into the buffer the live renderer reads, so the
//! CPU does nothing per frame; snapshots read the same buffer back.

use super::{Scene, Setup};
use crate::grid::Grid;
use anyhow::{Context, Result, bail};
use wgpu::util::DeviceExt;

const PRELUDE: &str = include_str!("shaders/prelude.wgsl");
const SHAPE: &str = include_str!("shaders/shape.wgsl");
const FRAME_SIZE: usize = 32 + 64;
const WORKGROUP: u32 = 64;

pub struct Spec<'a> {
    pub name: &'a str,
    pub src: &'a str,
    /// The scene defines `field(p)` and glyphs come from shape matching;
    /// otherwise it defines `cell(c)` and picks glyphs itself.
    pub shaped: bool,
    /// Harri's global contrast exponent applied to each cell's ink vector.
    pub contrast: f32,
    /// Glyph vocabulary per material (space is always allowed).
    pub masks: [&'a str; 4],
    /// Built-in scenes may attach words for binding 3. Packs cannot.
    pub data: ShaderData,
    /// Point scenes: how many times the scatter pass calls `points(i)` per frame.
    pub points: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShaderData {
    None,
    SourcePage,
}

pub const SPECS: &[Spec] = &[
    Spec {
        name: "wave",
        src: include_str!("shaders/wave.wgsl"),
        shaped: false,
        contrast: 1.0,
        masks: ["", "", "", ""],
        data: ShaderData::None,
        points: 0,
    },
    Spec {
        name: "singularity",
        src: include_str!("shaders/singularity.wgsl"),
        shaped: true,
        contrast: 1.15,
        masks: ["-_=~.*#%@", "(/\\|-_=.,'`", ".'`*+", ")/\\|-_=.,'`"],
        data: ShaderData::None,
        points: 0,
    },
    Spec {
        name: "aurora",
        src: include_str!("shaders/aurora.wgsl"),
        shaped: false,
        contrast: 1.0,
        masks: ["", "", "", ""],
        data: ShaderData::None,
        points: 0,
    },
    Spec {
        name: "blossom",
        src: include_str!("shaders/blossom.wgsl"),
        shaped: true,
        contrast: 1.4,
        masks: ["/\\|_-=()`',.", "*@&%oO0", "',.`*", ""],
        data: ShaderData::None,
        points: 0,
    },
    Spec {
        name: "koi",
        src: include_str!("shaders/koi.wgsl"),
        shaped: true,
        contrast: 1.3,
        masks: ["-_=~/\\|()<>.,'`:;", "oO0@*", "", ""],
        data: ShaderData::None,
        points: 3 * 9080 + 8 * 700,
    },
    Spec {
        name: "medusa",
        src: include_str!("shaders/medusa.wgsl"),
        shaped: true,
        contrast: 1.3,
        masks: ["-_=~/\\|()<>.,'`:;", "", "", ""],
        data: ShaderData::None,
        points: 22 * 300 + 4 * 500 + 14 * 600 + 4 * 1800 + 1500,
    },
    Spec {
        name: "glass",
        src: include_str!("shaders/glass.wgsl"),
        shaped: false,
        contrast: 1.0,
        masks: ["", "", "", ""],
        data: ShaderData::SourcePage,
        points: 0,
    },
    Spec {
        name: "saturn",
        src: include_str!("shaders/saturn.wgsl"),
        shaped: false,
        contrast: 1.3,
        masks: [".:;+*o#%@", "-_=~().'", "", ""],
        data: ShaderData::None,
        points: 0,
    },
    Spec {
        name: "dandelion",
        src: include_str!("shaders/dandelion.wgsl"),
        shaped: true,
        contrast: 1.4,
        masks: ["/\\|-_.'", "*+.'", "@O0o", "|/\\()"],
        data: ShaderData::None,
        points: 0,
    },
    Spec {
        name: "moonsea",
        src: include_str!("shaders/moonsea.wgsl"),
        shaped: false,
        contrast: 1.0,
        masks: ["", "", "", ""],
        data: ShaderData::None,
        points: 0,
    },
];

pub fn spec(name: &str) -> Option<&'static Spec<'static>> {
    SPECS.iter().find(|s| s.name == name)
}

impl<'a> Spec<'a> {
    pub fn source(&self) -> &str {
        self.src
    }

    fn module_source(&self) -> String {
        let shape = if self.shaped { SHAPE } else { "" };
        let prepare = if self.src.contains("fn prepare(") {
            ""
        } else {
            "fn prepare(lid: u32) {}\n"
        };
        let points = if self.src.contains("fn points(") {
            ""
        } else {
            "fn points(i: u32) {}\n"
        };
        format!("{PRELUDE}\n{shape}\n{prepare}{points}{}", self.src)
    }
}

/// Glyph bitmask (95 bits in 3 words) for a set of ASCII characters; space excluded.
fn mask(chars: &str) -> [u32; 4] {
    let mut m = [0u32; 4];
    for b in chars.bytes().filter(|b| (33..127).contains(b)) {
        let g = (b - 32) as usize;
        m[g / 32] |= 1 << (g % 32);
    }
    m
}

/// The scene's own WGSL as a page of cells for `glass`: `[lines, width, cells...]`,
/// each cell `byte | class << 8` (0 code, 1 keyword, 2 comment, 3 number).
fn source_page(spec: &Spec) -> Vec<u32> {
    const WIDTH: usize = 84; // glass.wgsl COL_W
    const KEYWORDS: [&str; 14] = [
        "fn", "let", "var", "return", "if", "else", "for", "while", "const", "struct", "loop",
        "break", "select", "mix",
    ];
    let src = spec.module_source();
    let lines: Vec<&str> = src.lines().collect();
    let mut out = Vec::with_capacity(2 + lines.len() * WIDTH);
    out.extend([lines.len() as u32, WIDTH as u32]);
    for line in &lines {
        let bytes: Vec<u8> = line
            .bytes()
            .map(|b| if (32..127).contains(&b) { b } else { b' ' })
            .collect();
        let comment = line.find("//").unwrap_or(usize::MAX);
        let mut class = vec![0u32; bytes.len()];
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if i >= comment {
                class[i] = 2;
                i += 1;
            } else if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                if KEYWORDS.contains(&&line[start..i]) {
                    class[start..i].fill(1);
                }
            } else if b.is_ascii_digit() {
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.') {
                    class[i] = 3;
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        out.extend((0..WIDTH).map(|x| bytes.get(x).map_or(32, |&b| b as u32 | class[x] << 8)));
    }
    out
}

fn words_to_bytes<T: Copy>(words: &[T], f: impl Fn(T) -> [u8; 4]) -> Vec<u8> {
    words.iter().flat_map(|w| f(*w)).collect()
}

pub struct ShaderScene {
    device: wgpu::Device,
    queue: wgpu::Queue,
    serial: std::sync::Arc<std::sync::Mutex<()>>,
    pipeline: wgpu::ComputePipeline,
    /// Point scenes: the scatter pipeline, its density buffer and workgroup count.
    scatter: Option<(wgpu::ComputePipeline, wgpu::Buffer, u32)>,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    cells: wgpu::Buffer,
    groups: u32,
}

impl ShaderScene {
    pub fn new(spec: &Spec, s: &Setup) -> Result<ShaderScene> {
        let gpu = s.gpu.get()?;
        let device = &gpu.device;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(spec.name),
            source: wgpu::ShaderSource::Wgsl(spec.module_source().into()),
        });
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some(spec.name),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, false),
                storage(3, true),
                storage(4, false),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(spec.name),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let entry = |name| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(spec.name),
                layout: Some(&layout),
                module: &module,
                entry_point: Some(name),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let pipeline = entry("main");
        let scatter_pipeline = (spec.points > 0).then(|| entry("scatter"));
        if let Some(e) = pollster::block_on(scope.pop()) {
            bail!("shader scene {}: {e}", spec.name);
        }

        let pairs = (s.cols * s.rows).div_ceil(2);
        let mut frame = [0u8; FRAME_SIZE];
        let masks = spec.masks.map(mask);
        let words: [u32; 8] = [
            s.cols,
            s.rows,
            s.seed as u32,
            pairs,
            0,
            s.aspect.to_bits(),
            spec.contrast.to_bits(),
            spec.points,
        ];
        for (i, w) in words.iter().chain(masks.as_flattened()).enumerate() {
            frame[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        let buffer = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let uniform = buffer(
            "frame",
            &frame,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let shapes = buffer(
            "shapes",
            &words_to_bytes(s.atlas.shapes().as_flattened(), f32::to_le_bytes),
            wgpu::BufferUsages::STORAGE,
        );
        let words = match spec.data {
            ShaderData::None => vec![0],
            ShaderData::SourcePage => source_page(spec),
        };
        let data = buffer(
            "data",
            &words_to_bytes(&words, u32::to_le_bytes),
            wgpu::BufferUsages::STORAGE,
        );
        let cells = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cells"),
            size: pairs as u64 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        // Two words (hits, tone sum) per density sample; a stub for scenes without points.
        let samples = if spec.points > 0 {
            s.cols as u64 * 2 * s.rows as u64 * 3
        } else {
            1
        };
        let dens = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("density"),
            size: samples * 8,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(spec.name),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: shapes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: cells.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: data.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: dens.as_entire_binding(),
                },
            ],
        });
        Ok(ShaderScene {
            device: device.clone(),
            queue: gpu.queue.clone(),
            serial: gpu.serial.clone(),
            pipeline,
            scatter: scatter_pipeline.map(|p| (p, dens, spec.points.div_ceil(WORKGROUP))),
            bind_group,
            uniform,
            cells,
            groups: pairs.div_ceil(WORKGROUP),
        })
    }

    /// Packed cells written by `encode`, two per u32 (same byte layout as `Grid`).
    pub fn cells(&self) -> &wgpu::Buffer {
        &self.cells
    }

    /// Record the compute passes for time `t`.
    pub fn encode(&self, enc: &mut wgpu::CommandEncoder, t: f32) {
        self.queue.write_buffer(&self.uniform, 16, &t.to_le_bytes());
        if let Some((pipeline, dens, groups)) = &self.scatter {
            enc.clear_buffer(dens, 0, None);
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(*groups, 1, 1);
        }
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.dispatch_workgroups(self.groups, 1, 1);
    }

    fn read_into(&self, t: f32, grid: &mut Grid) -> Result<()> {
        let size = self.cells.size();
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.encode(&mut enc, t);
        enc.copy_buffer_to_buffer(&self.cells, 0, &readback, 0, size);
        let _serial = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        self.queue.submit([enc.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |_| ());
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .context("GPU readback")?;
        let view = readback.get_mapped_range(..).context("GPU readback")?;
        let out = grid.cells.as_flattened_mut();
        let n = out.len();
        out.copy_from_slice(&view[..n]);
        Ok(())
    }
}

impl Scene for ShaderScene {
    fn update(&mut self, t: f32, _dt: f32, grid: &mut Grid) {
        if let Err(e) = self.read_into(t, grid) {
            eprintln!("asciiwall: {e:#}");
        }
    }

    fn gpu(&self) -> Option<&ShaderScene> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_sets_glyph_bits_and_skips_space() {
        let m = mask(" !~");
        assert_eq!(m[0], 1 << 1); // '!' = glyph 1
        assert_eq!(m[2], 1 << (94 - 64)); // '~' = glyph 94
        assert_eq!(m[1] | m[3], 0);
    }
}
