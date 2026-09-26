// Shared by every shader scene. A scene appends either `fn field(p) -> vec3<f32>`
// (plus shape.wgsl, which turns fields into glyphs) or its own `fn cell(c) -> u32`,
// and may define `fn prepare(lid: u32)` for per-workgroup setup. Point scenes also
// define `fn points(i: u32)`: a scatter pass runs it F.points times per frame, each
// call `plot`s into a density grid of 2x3 samples per cell that `field` reads back
// with `splat` (so a cloud of points becomes strokes through shape matching).
// One invocation fills two cells; output is packed like the CPU grid bytes:
// per cell `glyph_index | palette_slot << 8`, two cells per u32.

struct Frame {
    cols: u32,
    rows: u32,
    seed: u32,
    pairs: u32,
    t: f32,
    aspect: f32,
    contrast: f32,
    points: u32,
    masks: array<vec4<u32>, 4>,
};

@group(0) @binding(0) var<uniform> F: Frame;
@group(0) @binding(1) var<storage, read> shapes: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read_write> cells: array<u32>;
@group(0) @binding(3) var<storage, read> data: array<u32>;
// Point density: per sample, [hits, tone sum * 64].
@group(0) @binding(4) var<storage, read_write> dens: array<atomic<u32>>;

const PI: f32 = 3.14159265;
const TAU: f32 = 6.28318531;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(local_invocation_index) lid: u32) {
    // Per-workgroup setup shared by all 64 invocations (e.g. geometry in workgroup memory).
    prepare(lid);
    workgroupBarrier();
    let k = id.x;
    if (k >= F.pairs) {
        return;
    }
    let i = 2u * k;
    let a = cell(vec2<u32>(i % F.cols, i / F.cols));
    var b = 0u;
    if (i + 1u < F.cols * F.rows) {
        b = cell(vec2<u32>((i + 1u) % F.cols, (i + 1u) / F.cols));
    }
    cells[k] = (a & 0xffffu) | (b << 16u);
}

@compute @workgroup_size(64)
fn scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x < F.points) {
        points(id.x);
    }
}

// Density sample (2x3 per cell) under scene point `p`, or -1 off screen.
fn sample_index(p: vec2<f32>) -> i32 {
    let uv = vec2<f32>(p.x / F.aspect + 0.5, 0.5 - p.y);
    let s = vec2<i32>(floor(uv * vec2<f32>(f32(F.cols * 2u), f32(F.rows * 3u))));
    if (s.x < 0 || s.y < 0 || s.x >= i32(F.cols * 2u) || s.y >= i32(F.rows * 3u)) {
        return -1;
    }
    return s.y * i32(F.cols * 2u) + s.x;
}

// Deposit one point with a tone (tone > 1 = highlight).
fn plot(p: vec2<f32>, tone: f32) {
    let i = sample_index(p);
    if (i >= 0) {
        atomicAdd(&dens[2 * i], 1u);
        atomicAdd(&dens[2 * i + 1], u32(clamp(tone, 0.0, 3.0) * 64.0));
    }
}

// (hits, mean tone) at `p`; hits are scaled to the 256x67 reference grid so a
// scene looks equally dense on every monitor.
fn splat(p: vec2<f32>) -> vec2<f32> {
    let i = sample_index(p);
    if (i < 0) {
        return vec2<f32>(0.0);
    }
    let n = atomicLoad(&dens[2 * i]);
    if (n == 0u) {
        return vec2<f32>(0.0);
    }
    let tone = f32(atomicLoad(&dens[2 * i + 1])) / (64.0 * f32(n));
    return vec2<f32>(f32(n) * f32(F.cols * F.rows) / 17152.0, tone);
}

// Scene space: origin at screen center, y up, 1 unit = screen height.
fn to_p(c: vec2<u32>, local: vec2<f32>) -> vec2<f32> {
    let uv = (vec2<f32>(c) + local) / vec2<f32>(f32(F.cols), f32(F.rows));
    return vec2<f32>((uv.x - 0.5) * F.aspect, 0.5 - uv.y);
}

// Size of one character cell in scene units.
fn cell_size() -> vec2<f32> {
    return vec2<f32>(F.aspect / f32(F.cols), 1.0 / f32(F.rows));
}

// Ink of a stroke at distance `d` (scene units) from its centerline, whose
// normal is `n`. Width is set in cell units (half a cell across columns, a
// sixth of a cell across rows), so strokes land as | / - . ' like hand-set ASCII.
fn stroke(d: f32, n: vec2<f32>) -> f32 {
    let cs = cell_size();
    let w = abs(n.x) * 0.7 * cs.x + abs(n.y) * 0.15 * cs.y;
    return pow(w / (d + w), 2.5);
}

// ASCII byte + palette slot -> packed cell.
fn pack(ch: u32, slot: u32) -> u32 {
    return (ch - 32u) | (slot << 8u);
}

// tone in [0,1] -> ramp slot 1..6; tone > 1 -> highlight slot 7.
fn tone_slot(tone: f32) -> u32 {
    if (tone > 1.0) {
        return 7u;
    }
    return 1u + min(5u, u32(max(tone, 0.0) * 6.0));
}

fn rot(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(c, s, -s, c);
}

fn hash11(x: f32) -> f32 {
    return fract(sin(x * 127.1 + f32(F.seed % 1024u) * 0.618) * 43758.5453);
}

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21) + f32(F.seed % 1024u) * 0.137);
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
