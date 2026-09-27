// moonsea — a full moon over a night sea, set character by character the way
// Joan Stark and Rowan Crawford teach hand-made ASCII art: every glyph chosen
// for its shape and its height in the cell. The moon's rim is a pen line of
// _ . - ' / \ ( ) |, its highlands a half-tone of dots around dark maria and a
// few o O craters, ringed by a faint dotted halo; thin wisps of ~ - _ drift
// past it. Below a clean horizon, rows of wavelets (~~ -~~~- .-~~~-.) lengthen
// and thin out toward the viewer, and the moon's glitter path, a widening
// column of - ~ = *, shimmers with them. Composition and formulas are my own.

const HORIZON: f32 = 0.62; // horizon, as a fraction of the height from the top
const MOON_Y: f32 = 0.2;
const MOON_R: f32 = 0.15;
const HALO_R: f32 = 1.46; // outer halo ring, in moon radii

const C_QUOTE: u32 = 39u;
const C_LPAREN: u32 = 40u;
const C_RPAREN: u32 = 41u;
const C_STAR: u32 = 42u;
const C_PLUS: u32 = 43u;
const C_DASH: u32 = 45u;
const C_DOT: u32 = 46u;
const C_SLASH: u32 = 47u;
const C_EQ: u32 = 61u;
const C_BIGO: u32 = 79u;
const C_BSLASH: u32 = 92u;
const C_UNDER: u32 = 95u;
const C_TICK: u32 = 96u;
const C_BAR: u32 = 124u;
const C_TILDE: u32 = 126u;
const C_O: u32 = 111u;

// A small sloop on the horizon, 6x3 cells (32 = transparent).
const BOAT: array<u32, 18> = array<u32, 18>(
    32u, 32u, 124u, 92u, 32u, 32u,
    32u, 32u, 124u, 95u, 92u, 32u,
    92u, 95u, 95u, 95u, 95u, 47u,
);

// Maria in moon units (radius 1, y up): center, radii.
const MARIA: array<vec4<f32>, 6> = array<vec4<f32>, 6>(
    vec4<f32>(-0.3, 0.38, 0.3, 0.22),
    vec4<f32>(0.16, 0.42, 0.16, 0.15),
    vec4<f32>(0.32, 0.1, 0.2, 0.16),
    vec4<f32>(0.68, 0.32, 0.1, 0.12),
    vec4<f32>(-0.6, -0.05, 0.2, 0.36),
    vec4<f32>(-0.12, -0.32, 0.18, 0.13),
);

// Craters: center in moon units, size (> 0.5 = large, O).
const CRATERS: array<vec3<f32>, 4> = array<vec3<f32>, 4>(
    vec3<f32>(-0.12, -0.66, 1.0),
    vec3<f32>(-0.3, 0.06, 0.0),
    vec3<f32>(-0.56, 0.14, 0.0),
    vec3<f32>(0.4, -0.46, 0.0),
);

fn side() -> f32 {
    return select(1.0, -1.0, (F.seed & 1u) == 1u);
}

fn moon_center() -> vec2<f32> {
    return vec2<f32>(0.16 * side(), MOON_Y);
}

// Scene point -> continuous cell coordinates (columns right, rows down).
fn to_cells(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>((p.x / F.aspect + 0.5) * f32(F.cols), (0.5 - p.y) * f32(F.rows));
}

// A line crossing this cell's column at height f (0 top .. 1 bottom) with slope
// s (rows per column, rows grow downward), set the way an ASCII artist inks
// it: flat stretches by height (- _, a flat line high in the cell is the _ of
// the cell above), gentle slopes as ' - ., diagonals as .' pairs and steep
// ones as / \. 0 = the line isn't in this cell.
fn pen(f: f32, s: f32) -> u32 {
    let a = abs(s);
    if (a < 0.15) {
        if (f >= 0.3 && f < 0.72) {
            return C_DASH;
        }
        if (f >= 0.72 && f < 1.3) {
            return C_UNDER;
        }
        return 0u;
    }
    if (f < 0.0 || f >= 1.0) {
        return 0u;
    }
    if (a < 0.36) {
        if (f < 0.34) {
            return C_QUOTE;
        }
        if (f < 0.66) {
            return C_DASH;
        }
        return C_DOT;
    }
    if (a < 0.62) {
        return select(C_QUOTE, C_DOT, f >= 0.5);
    }
    return select(C_BSLASH, C_SLASH, s < 0.0);
}

// Pen line along an ellipse (center m, radii r, in cell units) through cell q:
// shallow arcs are found column by column and inked with `pen`, steep ones row
// by row as / \, rounding into ( ) and | at the sides.
fn rim(q: vec2<f32>, m: vec2<f32>, r: vec2<f32>) -> u32 {
    let u = (q.x + 0.5 - m.x) / r.x;
    if (abs(u) < 1.0) {
        let root = sqrt(1.0 - u * u);
        let sl = r.y * u / (r.x * max(root, 1e-3));
        for (var i = 0; i < 2; i++) {
            let sg = f32(2 * i - 1); // upper arc, then lower arc
            let s = -sg * sl;
            if (abs(s) < 1.0) {
                let g = pen(m.y + sg * r.y * root - q.y, s);
                if (g != 0u) {
                    return g;
                }
            }
        }
    }
    let v = (q.y + 0.5 - m.y) / r.y;
    if (abs(v) < 1.0) {
        let root = sqrt(1.0 - v * v);
        let sl = r.x * v / (r.y * max(root, 1e-3));
        for (var i = 0; i < 2; i++) {
            let sg = f32(2 * i - 1); // left arc, then right arc
            let f = m.x + sg * r.x * root - q.x;
            let d = -sg * sl; // columns per row
            if (abs(d) <= 1.0 && f >= 0.0 && f < 1.0) {
                if (abs(d) < 0.18) {
                    return C_BAR;
                }
                if (abs(d) < 0.5) {
                    return select(C_RPAREN, C_LPAREN, i == 0);
                }
                return select(C_BSLASH, C_SLASH, d < 0.0);
            }
        }
    }
    return 0u;
}

// Maria in moon units (radius 1, y up): soft blobs with ragged edges, 0..1.
fn mare(mp: vec2<f32>) -> f32 {
    let maria = MARIA;
    let wobble = 0.25 * (noise(mp * 4.0 + 11.0) - 0.5);
    var v = 0.0;
    for (var i = 0; i < 6; i++) {
        let b = maria[i];
        v = max(v, smoothstep(1.0, 0.55, length((mp - b.xy) / b.zw) + wobble));
    }
    return v;
}

// One of `n` evenly spaced halo dots at radius `r` (cells) lands in cell q?
// Returns the dot's glyph (' high in the cell, . low) or 0.
fn halo(q: vec2<f32>, mc: vec2<f32>, r: vec2<f32>, n: f32, phase: f32) -> u32 {
    let d = q + 0.5 - mc;
    let a = atan2(-d.y / r.y, d.x / r.x);
    let i0 = round(a * n / TAU - phase);
    for (var k = -1.0; k <= 1.0; k += 1.0) {
        let ai = (i0 + k + phase) * TAU / n;
        let dot = mc + r * vec2<f32>(cos(ai), -sin(ai));
        if (all(floor(dot) == q)) {
            return select(C_TICK, C_DOT, fract(dot.y) > 0.5);
        }
    }
    return 0u;
}

// The moon: a bright rim, soft half-toned maria and a few craters, a faint
// halo of two dotted rings.
fn moon(q: vec2<f32>, p: vec2<f32>) -> u32 {
    let m = moon_center();
    let mc = to_cells(m);
    let cr = vec2<f32>(f32(F.cols) / F.aspect, f32(F.rows)) * MOON_R; // radius in cells
    let g = rim(q, mc, cr);
    if (g != 0u) {
        return pack(g, 7u);
    }
    let d = length(p - m) / MOON_R;
    if (d > 1.05) {
        let h1 = halo(q, mc, cr * 1.22, 44.0, 0.0);
        if (h1 != 0u) {
            return pack(h1, 2u);
        }
        let h2 = halo(q, mc, cr * HALO_R, 36.0, 0.5);
        if (h2 != 0u) {
            return pack(h2, 1u);
        }
        return 0u;
    }
    if (d > 0.9) {
        return 0u;
    }
    let craters = CRATERS;
    for (var i = 0; i < 4; i++) {
        let c = craters[i];
        if (all(floor(mc + cr * vec2<f32>(c.x, -c.y)) == q)) {
            return pack(select(C_O, C_BIGO, c.z > 0.5), 4u);
        }
    }
    // Half-tone: the highlands are a staggered lattice of dots; the maria,
    // ragged-edged, stay dark.
    let v = mare((p - m) / MOON_R);
    let checker = (u32(q.x) + u32(q.y)) % 2u == 0u;
    if (v < 0.5 && checker) {
        return pack(C_DOT, 5u);
    }
    return 0u;
}

// Thin wisps: two clouds of three stacked strands drifting slowly east. Each
// strand keeps to one row and is a streak of ~ thinning to - and then to a
// spaced - - at its ends, over a flat base of _; the one crossing the moon
// catches its light.
fn clouds(q: vec2<f32>, p: vec2<f32>) -> u32 {
    let rows = f32(F.rows);
    for (var i = 0; i < 2; i++) {
        let fi = f32(i);
        let yb = 0.41 - 0.22 * fi;
        let len = 0.42 + 0.16 * hash11(fi + 9.0);
        let span = F.aspect + 2.0 * len;
        let xc = fract(0.3 + 0.55 * fi + 0.3 * hash11(fi + 1.0) + F.t * (0.0021 + 0.0007 * fi) / span) * span - 0.5 * span;
        let u = (p.x - xc) / len;
        if (abs(u) > 0.5) {
            continue;
        }
        let j = q.y - floor((0.5 - yb) * rows); // strand: row above, middle, below
        if (abs(j) > 1.0) {
            continue;
        }
        let w = 2.0 * (u - 0.16 * j * (1.0 - 2.0 * fi)) / (1.0 - 0.3 * abs(j) - 0.15 * j);
        let e = 1.0 - w * w;
        let dens = e * (0.35 + 0.9 * noise(vec2<f32>(u * 9.0 + j * 3.1, fi * 9.1 + j * 2.0)));
        if (dens < 0.3 || (dens < 0.45 && u32(q.x) % 2u == 0u)) {
            continue;
        }
        let lit = exp(-pow(length(p - moon_center()) / (1.4 * MOON_R), 2.0));
        var g = select(C_DASH, C_TILDE, dens > 0.7);
        if (j > 0.0) {
            g = C_UNDER;
        }
        return pack(g, tone_slot(0.14 + 0.18 * dens + 0.75 * lit));
    }
    return 0u;
}

fn stars(q: vec2<f32>, p: vec2<f32>, hz: f32) -> u32 {
    if (length(p - moon_center()) < 1.9 * MOON_R || q.y > hz - 3.0) {
        return 0u;
    }
    let block = vec2<f32>(5.0, 3.0);
    let id = floor(q / block);
    let h = hash21(id * 1.31 + 7.0);
    if (h < 0.76) {
        return 0u;
    }
    let at = id * block + floor(vec2<f32>(hash21(id + 3.3), hash21(id + 9.1)) * block);
    if (any(at != q)) {
        return 0u;
    }
    let tw = 0.5 + 0.5 * sin(F.t * (0.4 + 0.6 * hash21(id + 5.5)) + h * 60.0);
    var g = C_DOT;
    if (h > 0.99) {
        g = C_STAR;
    } else if (h > 0.975) {
        g = C_PLUS;
    } else if (h > 0.9) {
        g = select(C_QUOTE, C_TICK, hash21(id + 1.7) > 0.5);
    }
    return pack(g, tone_slot(0.1 + 0.35 * (h - 0.76) / 0.24 + 0.2 * tw));
}

fn boat(q: vec2<f32>, hz: f32) -> u32 {
    let span = F.aspect + 0.2;
    let bx = fract(0.5 - 0.45 * side() / span + F.t * 0.0007 / span) * span - 0.5 * span;
    let bc = floor((bx / F.aspect + 0.5) * f32(F.cols));
    let l = vec2<f32>(q.x - bc, q.y - (hz - 3.0));
    if (l.x < 0.0 || l.y < 0.0 || l.x >= 6.0 || l.y >= 3.0) {
        return 0u;
    }
    let sprite = BOAT;
    let g = sprite[u32(l.y) * 6u + u32(l.x)];
    if (g == 32u) {
        return 0u;
    }
    return pack(g, 3u);
}

fn sea(q: vec2<f32>, p: vec2<f32>, hz: f32) -> u32 {
    let rs = f32(F.rows) - hz;
    let cw = cell_size().x;
    let k = q.y - hz;
    let s = (k + 0.5) / rs; // depth: 0 at the horizon, 1 at the bottom edge
    let drift = F.t * (0.0008 + 0.0045 * s); // parallax: near rows slide faster

    // Moon glitter: narrow at the horizon, widening toward the viewer.
    let gw = 0.01 + 0.13 * s;
    let gx = (p.x - moon_center().x) / gw;
    let env = exp(-gx * gx);
    if (env > 0.05) {
        let gs = max(0.008 + 0.05 * s, 2.5 * cw);
        let sp = noise(vec2<f32>((p.x + drift) / gs, k * 9.53 + 50.0 + F.t * 0.32));
        let g = env * smoothstep(0.3, 0.8, sp) * (0.8 + 0.2 * s);
        if (g > 0.85) {
            return pack(C_STAR, 7u);
        }
        if (g > 0.6) {
            return pack(C_EQ, 7u);
        }
        if (g > 0.42) {
            return pack(C_TILDE, 6u);
        }
        if (g > 0.25) {
            return pack(C_DASH, 5u);
        }
    }

    // Wavelets: one per slot along the row, longer and sparser toward the
    // viewer. Each swells and subsides by growing or shrinking about its
    // middle, so a wave never pops in or out; broad calm patches thin them.
    let period = 0.045 + 0.2 * s;
    let off = hash11(k * 1.7 + 3.1) * 64.0;
    let x = (p.x + drift) / period + off;
    let n = floor(x);
    let h = hash11(n * 0.37 + k * 5.1);
    let mid = 0.5 + 0.3 * (h - 0.5);
    let at = (n + mid - off) * period - drift; // the wavelet's middle on screen
    let calm = noise(vec2<f32>(at * 2.2 + 5.0, s * 2.5 + F.t * 0.01)) - 0.5;
    let swell = noise(vec2<f32>(n * 0.71, k * 3.9 + F.t * 0.025));
    let thr = 0.35 + 0.12 * s + 0.3 * calm;
    let half = 0.5 * (0.35 + 0.3 * hash11(n * 0.53 + k * 2.3)) * smoothstep(thr, thr + 0.25, swell);
    let e = (half - abs(x - n - mid)) * period / cw; // cells from the wavelet's nearer end
    if (e <= 0.0) {
        return 0u;
    }
    var g = C_TILDE;
    if (s > 0.55) {
        if (e < 1.0) {
            g = C_DOT;
        } else if (e < 2.0) {
            g = C_DASH;
        }
    } else if (e < 1.0 && (s > 0.22 || half * period / cw > 2.0)) {
        g = C_DASH;
    }
    return pack(g, tone_slot(0.12 + 0.34 * s + 0.5 * env));
}

fn cell(c: vec2<u32>) -> u32 {
    let q = vec2<f32>(c);
    let p = to_p(c, vec2<f32>(0.5));
    let hz = floor(HORIZON * f32(F.rows)); // first sea row
    if (q.y >= hz) {
        return sea(q, p, hz);
    }
    let b = boat(q, hz);
    if (b != 0u) {
        return b;
    }
    if (q.y == hz - 1.0) {
        let gx = (p.x - moon_center().x) / 0.03;
        return pack(C_UNDER, tone_slot(0.2 + 0.6 * exp(-gx * gx)));
    }
    let cl = clouds(q, p);
    if (cl != 0u) {
        return cl;
    }
    if (length(p - moon_center()) < MOON_R * (HALO_R + 0.1)) {
        return moon(q, p);
    }
    return stars(q, p, hz);
}
