// aurora — original ASCII nightscape: slowly folding boreal curtains above
// mountain pines and a lake, with stars and faint reflected streaks.

const HZ: f32 = -0.22; // shoreline (scene y)

const C_QUOTE: u32 = 39u;
const C_STAR: u32 = 42u;
const C_PLUS: u32 = 43u;
const C_COMMA: u32 = 44u;
const C_DASH: u32 = 45u;
const C_DOT: u32 = 46u;
const C_SLASH: u32 = 47u;
const C_COLON: u32 = 58u;
const C_EQ: u32 = 61u;
const C_BSLASH: u32 = 92u;
const C_CARET: u32 = 94u;
const C_UNDER: u32 = 95u;
const C_TICK: u32 = 96u;
const C_BAR: u32 = 124u;
const C_TILDE: u32 = 126u;

fn row_of(y: f32) -> f32 {
    return (0.5 - y) * f32(F.rows);
}

fn x_of_col(c: f32) -> f32 {
    return (c / f32(F.cols) - 0.5) * F.aspect;
}

fn side() -> f32 {
    return select(1.0, -1.0, (F.seed & 1u) == 1u);
}

// One curtain: a sheet hanging from a wavy hem, seen from below. Screen x of
// sheet coordinate s is s + folds(s); where the sheet turns toward the eye it
// bunches up (dx/ds small) and glows brighter, and its rays crowd together.
// Returns (brightness, rows above the hem).
fn ribbon(i: u32, x: f32, y: f32) -> vec2<f32> {
    let fi = f32(i);
    let t = F.t;
    let ph = hash11(fi * 7.31 + 1.7) * TAU;
    let cs = cell_size();
    var y0 = 0.0;
    var sweep = 0.1;
    var a1 = 0.2;
    var k1 = 2.6;
    var hgt = 0.12;
    var gain = 1.2;
    var fr = 55.0;
    if (i == 1u) {
        y0 = 0.22;
        sweep = 0.06;
        a1 = 0.14;
        k1 = 3.4;
        hgt = 0.07;
        gain = 0.75;
        fr = 70.0;
    }
    let a2 = 0.025;
    let k2 = 7.3;
    let f1 = ph + t * 0.045;
    let f2 = ph * 2.3 - t * 0.07;
    var s = x;
    for (var n = 0; n < 3; n++) {
        let g = s + a1 * sin(k1 * s + f1) + a2 * sin(k2 * s + f2) - x;
        let dg = 1.0 + a1 * k1 * cos(k1 * s + f1) + a2 * k2 * cos(k2 * s + f2);
        s -= g / dg;
    }
    let dx = 1.0 + a1 * k1 * cos(k1 * s + f1) + a2 * k2 * cos(k2 * s + f2);
    let crease = (1.0 - a1 * k1 - a2 * k2) / dx;
    let hem = y0 + sweep * sin(s * 1.7 + ph + t * 0.013)
        + 0.35 * a1 * sin(k1 * s + f1)
        + 0.01 * sin(s * 5.1 - t * 0.05);
    let h = y - hem;
    if (h < -1.0 * cs.y) {
        return vec2<f32>(0.0, h / cs.y);
    }
    let drift = t * 0.25;
    let n1 = noise(vec2<f32>(s * fr + drift, t * 0.06 + fi * 13.0));
    let n2 = noise(vec2<f32>(s * fr * 2.3 - drift * 1.3, t * 0.1 + 5.0 + fi * 7.0));
    let r = smoothstep(0.2, 0.9, 0.65 * n1 + 0.45 * n2);
    let wide = noise(vec2<f32>(s * 6.0 + fi * 3.0, t * 0.03));
    let env = 0.6 + 0.4 * sin(s * 1.2 - t * 0.021 + ph * 3.0);
    let hh = max(h, 0.0);
    let len = hgt * (0.5 + 0.5 * r + 0.9 * wide) * (0.5 + 0.5 * crease);
    let body = exp(-hh / len) * (0.65 + 0.35 * r);
    let cut = smoothstep(-0.5 * cs.y, 0.3 * cs.y, h);
    let v = gain * cut * (0.4 + 0.6 * crease) * env * body;
    return vec2<f32>(v, h / cs.y);
}

// (brightness, rows above the dominant hem)
fn aurora(x: f32, y: f32) -> vec2<f32> {
    let a = ribbon(0u, x, y);
    let b = ribbon(1u, x, y);
    let v = 1.0 - (1.0 - clamp(a.x, 0.0, 1.0)) * (1.0 - clamp(b.x, 0.0, 1.0));
    let hr = select(b.y, a.y, a.x >= b.x);
    return vec2<f32>(max(v, max(a.x, b.x)), hr);
}

// Mountain height above the shore at x: ridged fBm under a slow envelope that
// raises one side of the range.
fn mountain(x: f32) -> f32 {
    let u = x * side() / (0.5 * F.aspect); // -1..1, high side at +1
    let env = 0.35 + 0.65 * smoothstep(-0.9, 0.6, u) - 0.3 * smoothstep(0.7, 1.1, u);
    var sum = 0.0;
    var amp = 0.5;
    var f = 1.6;
    for (var i = 0; i < 4; i++) {
        let n = 1.0 - abs(2.0 * noise(vec2<f32>(x * f + 7.0, f32(i) * 3.3)) - 1.0);
        sum += amp * n * n;
        amp *= 0.45;
        f *= 2.3;
    }
    return env * 0.24 * sum;
}

fn ridge(x: f32) -> f32 {
    return HZ + 0.02 + mountain(x);
}

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

// Mountain ridge glyph for cell q, or 0.
fn ridge_line(q: vec2<f32>) -> u32 {
    let rl = row_of(ridge(x_of_col(q.x)));
    let rm = row_of(ridge(x_of_col(q.x + 0.5)));
    let rr = row_of(ridge(x_of_col(q.x + 1.0)));
    let s = rr - rl;
    if (abs(s) < 0.45 && !(rm < min(rl, rr) - 0.3)) {
        return pen(rm - q.y, s);
    }
    let top = min(min(rl, rr), rm);
    let bot = max(max(rl, rr), rm);
    if (q.y + 1.0 <= top || q.y >= bot) {
        return 0u;
    }
    if (rm < rl - 0.3 && rm < rr - 0.3 && floor(rm) == q.y) {
        return C_CARET;
    }
    return select(C_BSLASH, C_SLASH, s < 0.0);
}

fn stars(q: vec2<f32>) -> u32 {
    let block = vec2<f32>(5.0, 3.0);
    let id = floor(q / block);
    let h = hash21(id * 1.31 + 7.0);
    if (h < 0.74) {
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
    return pack(g, tone_slot(0.1 + 0.35 * (h - 0.74) / 0.26 + 0.2 * tw));
}

fn sky_glyph(q: vec2<f32>, a: vec2<f32>) -> u32 {
    let v = a.x;
    if (v < 0.08) {
        return 0u;
    }
    var slot = tone_slot(v * 0.95);
    if (v > 0.85 && a.y < 1.5) {
        slot = 7u;
    }
    if (a.y < 0.5 && v > 0.2) {
        return pack(select(C_BAR, C_QUOTE, a.y < 0.0), slot);
    }
    return pack(C_BAR, slot);
}

// Spruce in the /|\ idiom: ^ tip, | trunk, / and \ branches drooping to each
// side, in tiers of `tier` rows. Returns 0 outside, else glyph | slot << 8.
fn pine(q: vec2<f32>, cx: f32, tr: f32, tier: f32) -> u32 {
    let d = q.y - tr;
    if (d < 0.0) {
        return 0u;
    }
    let o = q.x - cx;
    if (d == 0.0) {
        return select(0u, C_CARET | (2u << 8u), o == 0.0);
    }
    let k = floor((d - 1.0) / tier);
    let m = d - 1.0 - k * tier;
    let w = 1.0 + k + m;
    if (abs(o) > w) {
        return 0u;
    }
    if (o == 0.0) {
        return C_BAR | (1u << 8u);
    }
    let edge = abs(o) == w;
    return select(C_BSLASH, C_SLASH, o < 0.0) | (select(1u, 2u, edge) << 8u);
}

const TREES: array<vec3<f32>, 5> = array<vec3<f32>, 5>(
    vec3<f32>(0.06, 0.28, 3.0),
    vec3<f32>(0.13, 0.46, 3.0),
    vec3<f32>(0.015, 0.5, 3.0),
    vec3<f32>(0.95, 0.5, 3.0),
    vec3<f32>(0.89, 0.62, 3.0),
);

// Foreground spruces, nearest first.
fn trees(q: vec2<f32>) -> u32 {
    let C = f32(F.cols);
    let R = f32(F.rows);
    let tt = TREES;
    for (var i = 0; i < 5; i++) {
        var u = tt[i].x;
        if (side() < 0.0) {
            u = 1.0 - u;
        }
        let g = pine(q, floor(C * u), floor(R * tt[i].y), tt[i].z);
        if (g != 0u) {
            return pack(g & 0xffu, g >> 8u);
        }
    }
    return 0u;
}

// A far treeline on the shore: small spruces one to five rows tall.
fn treeline(q: vec2<f32>, hzr: f32) -> u32 {
    let sp = 3.0;
    let i0 = floor(q.x / sp);
    for (var j = -2.0; j <= 2.0; j += 1.0) {
        let i = i0 + j;
        let h = hash11(i * 0.713 + 4.0);
        let lay = noise(vec2<f32>(i * 0.21, 8.0));
        let hgt = floor(1.0 + 4.5 * lay * lay + 1.5 * h);
        let cx = i * sp + floor(h * 2.0);
        let g = pine(q, cx, hzr - hgt, 2.0);
        if (g != 0u) {
            return pack(g & 0xffu, 1u);
        }
    }
    return 0u;
}

fn lake(q: vec2<f32>, p: vec2<f32>) -> u32 {
    let cs = cell_size();
    let depth = HZ - p.y;
    let yy = HZ + depth;
    let rip = (0.002 + 0.03 * depth) * sin(q.y * 1.7 + F.t * 0.6 + p.x * 2.0);
    let xx = p.x + rip;
    if (yy < ridge(xx) + 0.5 * cs.y) {
        return 0u;
    }
    let a = aurora(xx, yy);
    let v = a.x * (0.55 - 0.6 * depth);
    let w = noise(vec2<f32>(p.x * 30.0 + 3.0 * sin(F.t * 0.05), q.y * 0.9 + F.t * 0.04));
    if (v < 0.06 || w < 0.45) {
        return 0u;
    }
    var g = C_DASH;
    if (v > 0.35) {
        g = C_EQ;
    } else if (v > 0.2) {
        g = C_TILDE;
    }
    return pack(g, tone_slot(v * 0.9));
}

// Snow on the upper slopes: hatching along the fall line, / on slopes rising
// to the right, \ on the others, thinning out downhill.
fn snow(q: vec2<f32>, p: vec2<f32>) -> u32 {
    let cs = cell_size();
    let top = ridge(p.x);
    let hgt = top - HZ;
    let depth = (top - p.y) / max(hgt, 1e-3);
    let cap = 0.35 + 0.25 * noise(vec2<f32>(p.x * 20.0, 1.0));
    if (depth > cap || hgt < 0.06) {
        return 0u;
    }
    let slope = (ridge(p.x + cs.x) - ridge(p.x - cs.x)) / (2.0 * cs.x);
    let n = noise(vec2<f32>(p.x * 60.0 + p.y * 20.0 * sign(slope), 2.0));
    if (n < 0.3 + 0.7 * depth / cap) {
        return 0u;
    }
    return pack(select(C_BSLASH, C_SLASH, slope > 0.0), 3u);
}

fn cell(c: vec2<u32>) -> u32 {
    let q = vec2<f32>(c);
    let p = to_p(c, vec2<f32>(0.5));
    let tr = trees(q);
    if (tr != 0u) {
        return tr;
    }
    let hzr = floor(row_of(HZ));
    if (q.y > hzr) {
        return lake(q, p);
    }
    let tl = treeline(q, hzr);
    if (tl != 0u) {
        return tl;
    }
    let rg = ridge_line(q);
    if (rg != 0u) {
        return pack(rg, 4u);
    }
    if (p.y < ridge(p.x)) {
        return snow(q, p);
    }
    let a = aurora(p.x, p.y);
    let g = sky_glyph(q, a);
    if (g != 0u) {
        return g;
    }
    return stars(q);
}
