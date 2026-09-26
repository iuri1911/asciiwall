struct U {
    cell: vec2<u32>,
    grid: vec2<u32>,
    offset: vec2<u32>,
    pad: vec2<u32>,
    palette: array<vec4<f32>, 8>,
};

@group(0) @binding(0) var<uniform> u: U;
// Two cells per word, each `glyph | slot << 8` (the CPU grid's byte layout).
@group(0) @binding(1) var<storage, read> cells: array<u32>;
@group(0) @binding(2) var atlas: texture_2d<f32>;

// Full-screen triangle.
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(floor(pos.xy)) - vec2<i32>(u.offset);
    let cs = vec2<i32>(u.cell);
    let size = cs * vec2<i32>(u.grid);
    if (p.x < 0 || p.y < 0 || p.x >= size.x || p.y >= size.y) {
        return u.palette[0];
    }
    let c = p / cs;
    let l = p % cs;
    let i = u32(c.y) * u.grid.x + u32(c.x);
    let v = (cells[i >> 1u] >> ((i & 1u) * 16u)) & 0xffffu;
    let cov = textureLoad(atlas, vec2<i32>(i32(v & 0xffu) * cs.x + l.x, l.y), 0).r;
    return mix(u.palette[0], u.palette[v >> 8u], cov);
}
