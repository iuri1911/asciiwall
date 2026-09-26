// glass — after harshitlog's "bend images through glass" (x.com/harshitlog/
// status/2099485977853231443). The image under the glass is this shader's own
// source, set in columns and slowly scrolling. Characters can't be warped, so
// the lens remaps whole cells: inside, glyphs are fetched from magnified
// positions; the rim is drawn with the characters circles are made of.

const COL_W: i32 = 84;
const GAP: i32 = 6;

// Page cell at (column, line) of the flowed text: byte | class << 8.
fn page(x: i32, y: i32) -> u32 {
    let lines = i32(data[0]);
    let width = i32(data[1]);
    let cols = max(1, (i32(F.cols) + GAP) / (COL_W + GAP));
    let margin = (i32(F.cols) - cols * (COL_W + GAP) + GAP) / 2;
    let xx = x - margin;
    let j = xx / (COL_W + GAP);
    let within = xx - j * (COL_W + GAP);
    if (xx < 0 || j >= cols || within >= width) {
        return 32u;
    }
    let scroll = i32(F.t * 0.5);
    let line = ((y + j * i32(F.rows) + scroll) % lines + lines) % lines;
    return data[2 + line * width + within];
}

const OUTSIDE: array<u32, 4> = array<u32, 4>(2u, 3u, 1u, 3u);
const INSIDE: array<u32, 4> = array<u32, 4>(5u, 7u, 3u, 6u);

fn cell(c: vec2<u32>) -> u32 {
    let n = vec2<f32>(f32(F.cols), f32(F.rows));
    let cw = F.aspect * n.y / n.x; // cell width in row heights
    let pc = vec2<f32>(c) + 0.5;
    for (var i = 0; i < 2; i++) {
        let fi = f32(i);
        let center = n * vec2<f32>(
            0.5 + (0.34 - 0.12 * fi) * sin(F.t * (0.061 + 0.03 * fi) + fi * 2.0),
            0.5 + (0.27 - 0.08 * fi) * sin(F.t * (0.047 + 0.021 * fi) + 1.0 + fi * 4.0)
        );
        let radius = n.y * (0.24 - 0.09 * fi);
        let d = (pc - center) * vec2<f32>(cw, 1.0);
        let r = length(d);
        let dir = d / max(r, 1e-3);
        // Rim: the circle's crossing cells, one per row at the sides, one per column on top.
        let thick = 0.42 * max(abs(dir.x) * cw, abs(dir.y));
        if (abs(r - radius) < thick) {
            let ax = abs(dir.x);
            var ch = 124u; // |
            if (ax < 0.38) {
                ch = select(95u, 45u, dir.y > 0.0); // _ top, - bottom
            } else if (ax < 0.92) {
                ch = select(92u, 47u, (dir.x > 0.0) == (dir.y > 0.0)); // \ or /
            }
            return pack(ch, 6u);
        }
        if (r < radius) {
            let k = r / radius;
            // Specular glint on the upper left.
            if (k > 0.72 && k < 0.86 && dir.x < -0.35 && dir.y < -0.35) {
                return pack(39u, 7u); // '
            }
            let src = center + (pc - center) * (0.45 + 0.55 * k * k);
            let v = page(i32(floor(src.x)), i32(floor(src.y)));
            if ((v & 0xffu) == 32u) {
                return 0u;
            }
            return pack(v & 0xffu, INSIDE[v >> 8u]);
        }
    }
    let v = page(i32(c.x), i32(c.y));
    if ((v & 0xffu) == 32u) {
        return 0u;
    }
    return pack(v & 0xffu, OUTSIDE[v >> 8u]);
}
