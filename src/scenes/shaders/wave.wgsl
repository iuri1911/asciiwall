// wave — original ASCII homage to Hokusai's public-domain "The Great Wave
// off Kanagawa" (c. 1831), https://www.metmuseum.org/art/collection/search/45434.
// Rolling Gerstner swells, a curling foam crest, drifting spray and distant Fuji.

const C_QUOTE: u32 = 39u;
const C_LPAREN: u32 = 40u;
const C_RPAREN: u32 = 41u;
const C_STAR: u32 = 42u;
const C_COMMA: u32 = 44u;
const C_DASH: u32 = 45u;
const C_DOT: u32 = 46u;
const C_SLASH: u32 = 47u;
const C_ZERO: u32 = 48u;
const C_COLON: u32 = 58u;
const C_AT: u32 = 64u;
const C_BIGO: u32 = 79u;
const C_BSLASH: u32 = 92u;
const C_CARET: u32 = 94u;
const C_UNDER: u32 = 95u;
const C_TICK: u32 = 96u;
const C_O: u32 = 111u;
const C_V: u32 = 118u;
const C_BAR: u32 = 124u;
const C_TILDE: u32 = 126u;

const NO: u32 = 64u; // vertices of the outer surface (back, crest, lip), ending at the tip
const NI: u32 = 64u; // vertices of the inner surface (underside, face), starting at the tip
const NCLAW: u32 = 9u;
const CP: u32 = 7u; // points per claw

var<workgroup> outer: array<vec4<f32>, 64>; // xy, arc length, 0
var<workgroup> inner: array<vec4<f32>, 64>;
var<workgroup> claws: array<vec4<f32>, 63>; // xy, radius, 0

fn cyc(period: f32) -> f32 {
    return fract(F.t / period);
}

fn wob(period: f32, off: f32) -> f32 {
    return sin(TAU * fract(F.t / period + off));
}

fn sq(x: f32) -> f32 {
    return x * x;
}

// Control points of the outer surface, from off-screen left to the lip's tip.
fn outer_ctrl(i: i32) -> vec2<f32> {
    let pts = array<vec2<f32>, 12>(
        vec2<f32>(-1.00, -0.12),
        vec2<f32>(-0.85, -0.06),
        vec2<f32>(-0.70, 0.04),
        vec2<f32>(-0.58, 0.16),
        vec2<f32>(-0.47, 0.28),
        vec2<f32>(-0.34, 0.36),
        vec2<f32>(-0.20, 0.38),
        vec2<f32>(-0.08, 0.34),
        vec2<f32>(0.00, 0.26),
        vec2<f32>(0.01, 0.18),
        vec2<f32>(-0.03, 0.12),
        vec2<f32>(-0.08, 0.10),
    );
    let k = clamp(i, 0, 11);
    var p = pts[k];
    // The lip breathes: reaching out and curling under, slowly.
    let reach = wob(37.0, 0.0);
    let w = smoothstep(6.0, 10.0, f32(k));
    p += w * vec2<f32>(0.02 * reach, -0.01 * reach);
    return p;
}

fn inner_ctrl(i: i32) -> vec2<f32> {
    let pts = array<vec2<f32>, 13>(
        vec2<f32>(-0.08, 0.10),
        vec2<f32>(-0.07, 0.15),
        vec2<f32>(-0.09, 0.22),
        vec2<f32>(-0.15, 0.26),
        vec2<f32>(-0.22, 0.25),
        vec2<f32>(-0.27, 0.19),
        vec2<f32>(-0.29, 0.10),
        vec2<f32>(-0.28, 0.00),
        vec2<f32>(-0.24, -0.12),
        vec2<f32>(-0.15, -0.24),
        vec2<f32>(0.00, -0.32),
        vec2<f32>(0.20, -0.38),
        vec2<f32>(0.45, -0.42),
    );
    let k = clamp(i, 0, 12);
    var p = pts[k];
    let reach = wob(37.0, 0.0);
    let w = 1.0 - smoothstep(1.0, 4.0, f32(k));
    p += w * vec2<f32>(0.02 * reach, -0.01 * reach);
    return p;
}

fn catmull(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, d: vec2<f32>, t: f32) -> vec2<f32> {
    let t2 = t * t;
    let t3 = t2 * t;
    return 0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3);
}

fn outer_at(u: f32) -> vec2<f32> {
    let x = clamp(u, 0.0, 1.0) * 11.0;
    let i = min(i32(x), 10);
    return catmull(outer_ctrl(i - 1), outer_ctrl(i), outer_ctrl(i + 1), outer_ctrl(i + 2), x - f32(i));
}

fn inner_at(u: f32) -> vec2<f32> {
    let x = clamp(u, 0.0, 1.0) * 12.0;
    let i = min(i32(x), 11);
    return catmull(inner_ctrl(i - 1), inner_ctrl(i), inner_ctrl(i + 1), inner_ctrl(i + 2), x - f32(i));
}

fn prepare(lid: u32) {
    outer[lid] = vec4<f32>(outer_at(f32(lid) / f32(NO - 1u)), 0.0, 0.0);
    inner[lid] = vec4<f32>(inner_at(f32(lid) / f32(NI - 1u)), 0.0, 0.0);
    if (lid < NCLAW) {
        // Foam fingers along the lip's leading edge: each reaches out and
        // hooks down and back like a claw, flexing in a wave down the row.
        let j = f32(lid);
        let u = (j + 0.5) / f32(NCLAW);
        let at = mix(0.62, 0.985, u);
        let base = outer_at(at);
        let tg = normalize(outer_at(at + 0.004) - base);
        let out = vec2<f32>(-tg.y, tg.x);
        let pulse = sin(TAU * (cyc(9.0) - u * 0.9));
        let len = (0.045 + 0.035 * u) * (1.0 + 0.12 * pulse);
        let bend = 3.6 + 0.5 * pulse;
        var a = atan2(out.y + tg.y * 0.5, out.x + tg.x * 0.5);
        var pt = base;
        claws[lid * CP] = vec4<f32>(pt, 1.0, 0.0);
        for (var m = 1u; m < CP; m++) {
            let da = bend / f32(CP - 1u);
            a -= da * 0.5;
            pt += vec2<f32>(cos(a), sin(a)) * len / f32(CP - 1u);
            a -= da * 0.5;
            claws[lid * CP + m] = vec4<f32>(pt, 1.0 - f32(m) / f32(CP - 1u), 0.0);
        }
    }
    workgroupBarrier();
    if (lid == 0u) {
        var s = 0.0;
        for (var i = 1u; i < NO; i++) {
            s += length(outer[i].xy - outer[i - 1u].xy);
            outer[i].z = s;
        }
    }
    if (lid == 1u) {
        var s = 0.0;
        for (var i = 1u; i < NI; i++) {
            s += length(inner[i].xy - inner[i - 1u].xy);
            inner[i].z = s;
        }
    }
}

fn to_cells(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>((p.x / F.aspect + 0.5) * f32(F.cols), (0.5 - p.y) * f32(F.rows));
}

// A line crossing this cell's column at height f (0 top .. 1 bottom) with slope
// s (rows per column, rows grow downward), inked the way an ASCII artist would.
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

// A straight line through the cell: signed distance d from the cell center
// along unit normal n. bend > 0: curving around a center to the right, ( .
fn pen_line(d: f32, n: vec2<f32>, bend: f32) -> u32 {
    let cs = cell_size();
    let a = n.x * cs.x;
    let b = -n.y * cs.y;
    if (abs(b) >= abs(a)) {
        return pen(0.5 - d / b, -a / b);
    }
    let u0 = -d / a;
    if (u0 < -0.5 || u0 >= 0.5) {
        return 0u;
    }
    let r = -b / a;
    if (abs(r) < 0.2) {
        return C_BAR;
    }
    if (abs(r) < 0.55 && bend != 0.0) {
        return select(C_RPAREN, C_LPAREN, bend > 0.0);
    }
    return select(C_BSLASH, C_SLASH, r < 0.0);
}

struct Near {
    d: f32, // distance
    n: vec2<f32>, // unit direction from the curve to p
    sd: f32, // signed distance along the segment's left normal
    e: vec2<f32>, // segment direction
    s: f32, // arc length at the foot
    bend: f32,
    end: bool, // the foot is a polyline end (no stroke there)
};

fn crossing(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> bool {
    return ((a.y > p.y) != (b.y > p.y)) && (p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x);
}

fn near_outer(p: vec2<f32>) -> Near {
    var best = 1e9;
    var bi = 0u;
    var bh = 0.0;
    for (var i = 0u; i + 1u < NO; i++) {
        let a = outer[i].xy;
        let e = outer[i + 1u].xy - a;
        let h = clamp(dot(p - a, e) / dot(e, e), 0.0, 1.0);
        let v = p - a - e * h;
        let d = dot(v, v);
        if (d < best) {
            best = d;
            bi = i;
            bh = h;
        }
    }
    let a = outer[bi].xy;
    let b = outer[bi + 1u].xy;
    let e = normalize(b - a);
    let foot = mix(a, b, bh);
    let dist = sqrt(best);
    let pa = outer[max(bi, 1u) - 1u].xy;
    let pb = outer[min(bi + 2u, NO - 1u)].xy;
    let cr = (b.x - pa.x) * (pb.y - a.y) - (b.y - pa.y) * (pb.x - a.x);
    let bend = select(0.0, sign(cr) * -e.y, abs(cr) > 2e-5);
    let nl = vec2<f32>(-e.y, e.x);
    let end = (bi == 0u && bh <= 0.0) || (bi == NO - 2u && bh >= 1.0);
    return Near(dist, (p - foot) / max(dist, 1e-6), dot(p - foot, nl), e, mix(outer[bi].z, outer[bi + 1u].z, bh), bend, end);
}

fn near_inner(p: vec2<f32>) -> Near {
    var best = 1e9;
    var bi = 0u;
    var bh = 0.0;
    for (var i = 0u; i + 1u < NI; i++) {
        let a = inner[i].xy;
        let e = inner[i + 1u].xy - a;
        let h = clamp(dot(p - a, e) / dot(e, e), 0.0, 1.0);
        let v = p - a - e * h;
        let d = dot(v, v);
        if (d < best) {
            best = d;
            bi = i;
            bh = h;
        }
    }
    let a = inner[bi].xy;
    let b = inner[bi + 1u].xy;
    let e = normalize(b - a);
    let foot = mix(a, b, bh);
    let dist = sqrt(best);
    let pa = inner[max(bi, 1u) - 1u].xy;
    let pb = inner[min(bi + 2u, NI - 1u)].xy;
    let cr = (b.x - pa.x) * (pb.y - a.y) - (b.y - pa.y) * (pb.x - a.x);
    let bend = select(0.0, sign(cr) * -e.y, abs(cr) > 2e-5);
    let nl = vec2<f32>(-e.y, e.x);
    let end = (bi == 0u && bh <= 0.0) || (bi == NI - 2u && bh >= 1.0);
    return Near(dist, (p - foot) / max(dist, 1e-6), dot(p - foot, nl), e, mix(inner[bi].z, inner[bi + 1u].z, bh), bend, end);
}

fn inside_wave(p: vec2<f32>) -> bool {
    var inside = false;
    for (var i = 0u; i + 1u < NO; i++) {
        if (crossing(p, outer[i].xy, outer[i + 1u].xy)) {
            inside = !inside;
        }
    }
    for (var i = 0u; i + 1u < NI; i++) {
        if (crossing(p, inner[i].xy, inner[i + 1u].xy)) {
            inside = !inside;
        }
    }
    let a = inner[NI - 1u].xy;
    let b = vec2<f32>(a.x, -1.0);
    let c = vec2<f32>(-1.2, -1.0);
    let d = outer[0].xy;
    if (crossing(p, a, b)) {
        inside = !inside;
    }
    if (crossing(p, b, c)) {
        inside = !inside;
    }
    if (crossing(p, c, vec2<f32>(c.x, d.y))) {
        inside = !inside;
    }
    if (crossing(p, vec2<f32>(c.x, d.y), d)) {
        inside = !inside;
    }
    return inside;
}

// Foam: bubbles of a drifting noise, big near the surface and sparse deeper.
fn foam_glyph(v: f32, q: vec2<f32>) -> u32 {
    if (v > 0.78) {
        return select(C_BIGO, C_AT, v > 0.9);
    }
    if (v > 0.6) {
        return C_O;
    }
    if (v > 0.45) {
        return select(C_DOT, C_QUOTE, u32(q.x + q.y) % 2u == 0u);
    }
    return 0u;
}

fn claw_cell(p: vec2<f32>, q: vec2<f32>) -> u32 {
    var cb = 1e9;
    var ci = 0u;
    var chh = 0.0;
    for (var i = 0u; i < NCLAW * CP; i++) {
        if (i % CP == CP - 1u) {
            continue;
        }
        let ca = claws[i].xy;
        let ce = claws[i + 1u].xy - ca;
        let h = clamp(dot(p - ca, ce) / dot(ce, ce), 0.0, 1.0);
        let v = p - ca - ce * h;
        let d = dot(v, v);
        if (d < cb) {
            cb = d;
            ci = i;
            chh = h;
        }
    }
    let r = mix(claws[ci].z, claws[ci + 1u].z, chh); // 1 at the root, 0 at the tip
    let cs = cell_size();
    let d = sqrt(cb);
    let w = cs.y * (0.35 + 0.5 * r);
    if (d > w) {
        return 0u;
    }
    if (r > 0.6) {
        return pack(select(C_BIGO, C_AT, r > 0.8), 7u);
    }
    if (r > 0.3) {
        return pack(C_O, 7u);
    }
    let ce = normalize(claws[ci + 1u].xy - claws[ci].xy);
    let cn = vec2<f32>(-ce.y, ce.x);
    let g = pen_line(dot(p - claws[ci].xy, cn), cn, ce.y);
    if (g != 0u) {
        return pack(g, 6u);
    }
    return 0u;
}

fn great_wave(q: vec2<f32>, p: vec2<f32>) -> u32 {
    let no = near_outer(p);
    let ni = near_inner(p);
    let inside = inside_wave(p);
    let lo = outer[NO - 1u].z;
    let rem = lo - no.s; // arc length to the lip's tip
    let crest = smoothstep(0.62, 0.42, rem);
    let cs = cell_size();

    // Foam cap on the crest, drifting toward the lip.
    let fw = 0.045 * crest;
    if (inside && no.d < fw && no.d <= ni.d) {
        let fz = noise(vec2<f32>((rem + F.t * 0.012) * 55.0, no.d * 70.0)) * 0.7 + 0.3 * noise(vec2<f32>(rem * 140.0 + F.t * 1.3, no.d * 150.0));
        let v = fz * (1.25 - 0.7 * no.d / fw);
        let g = foam_glyph(v, q);
        if (g != 0u) {
            return pack(g, select(6u, 7u, v > 0.78));
        }
    }
    // The two surfaces, inked.
    if (!no.end && no.d < 0.02 && no.d <= ni.d) {
        let go = pen_line(no.sd, vec2<f32>(-no.e.y, no.e.x), no.bend);
        if (go != 0u) {
            return pack(go, select(5u, 7u, crest > 0.5));
        }
    }
    if (!ni.end && ni.d < 0.02) {
        let gi = pen_line(ni.sd, vec2<f32>(-ni.e.y, ni.e.x), ni.bend);
        if (gi != 0u) {
            return pack(gi, 5u);
        }
    }
    let cl = claw_cell(p, q);
    if (cl != 0u) {
        return cl;
    }
    if (!inside) {
        return 0u;
    }
    // Streamlines of the body: level sets of v = do / (do + di), so they run
    // parallel to both surfaces and converge into the lip.
    let sum = no.d + ni.d;
    let v = no.d / sum;
    let grad = (ni.d * no.n - no.d * ni.n) / (sum * sum);
    let gl = max(length(grad), 1e-4);
    let nn = grad / gl;
    let K = 8.0;
    let k = round(v * K);
    if (k < 1.0 || k > K - 1.0) {
        return 0u;
    }
    // Spacing between neighbouring lines, in cells across them; thin out
    // where they converge.
    let across = 1.0 / (K * gl) / (abs(nn.y) * cs.y + abs(nn.x) * cs.x);
    if (across < 1.3 && (u32(k) % 2u == 1u || across < 0.65)) {
        return 0u;
    }
    let along = mix(rem, ni.s, v);
    let dash = fract(along / 0.16 + k * 0.29 - cyc(8.0));
    if (dash > 0.85) {
        return 0u;
    }
    let g = pen_line((v - k / K) / gl, nn, mix(no.bend, ni.bend, v));
    if (g == 0u) {
        return 0u;
    }
    return pack(g, tone_slot(0.55 - 0.3 * sin(PI * v)));
}

// Foreground sea: a long swell train rolling right through a fixed envelope,
// so crests rise, peak and settle as they pass. Layer j is j stripes deep;
// deeper layers feel less of the swell.
fn sea_env(x: f32) -> f32 {
    return 0.028 + 0.05 * exp(-sq((x + 0.5) / 0.25)) + 0.035 * exp(-sq((x - 0.72) / 0.2));
}

fn sea_base(x: f32) -> f32 {
    return -0.37 + 0.2 * exp(-sq((x - 0.82) / 0.3)) + 0.08 * exp(-sq((x + 0.6) / 0.3));
}

const SEA_DEPTH: array<f32, 5> = array<f32, 5>(0.0, 0.03, 0.065, 0.105, 0.15);

// Gerstner (trochoidal) swell: sharp crests, broad troughs. Returns the
// surface offset and the crest proximity (1 on a crest).
fn swell(x: f32, a: f32) -> vec2<f32> {
    let k = TAU / 0.42;
    let ph = TAU * cyc(24.0);
    var x0 = x;
    for (var i = 0; i < 4; i++) {
        x0 = x + 0.85 * a * sin(k * x0 - ph);
    }
    let c = cos(k * x0 - ph);
    return vec2<f32>(a * c, c);
}

fn sea_layer(x: f32, j: u32) -> f32 {
    let a = sea_env(x) * exp(-f32(j) * 0.45);
    return sea_base(x) - SEA_DEPTH[j] + swell(x, a).x;
}

fn sea_h(x: f32) -> f32 {
    return sea_layer(x, 0u);
}

fn sea(q: vec2<f32>, p: vec2<f32>, h: f32) -> u32 {
    let cs = cell_size();
    for (var j = 0u; j < 5u; j++) {
        let y = sea_layer(p.x, j);
        if (p.y > y + cs.y) {
            continue;
        }
        let slope = (sea_layer(p.x + cs.x * 0.5, j) - sea_layer(p.x - cs.x * 0.5, j)) / cs.x;
        let n = normalize(vec2<f32>(-slope, 1.0));
        if (j > 0u) {
            let dash = fract(p.x / (0.1 + 0.04 * f32(j)) + f32(j) * 0.37 - cyc(40.0 + 9.0 * f32(j)));
            if (dash > 0.8) {
                continue;
            }
        }
        let g = pen_line((p.y - y) * n.y, n, 0.0);
        if (g != 0u) {
            return pack(g, select(tone_slot(0.55 - 0.1 * f32(j)), 6u, j == 0u));
        }
        if (j == 0u && p.y < y) {
            // Foam just under a crest.
            let sw = swell(p.x, sea_env(p.x));
            let crest = smoothstep(0.75, 0.98, sw.y) * smoothstep(0.03, 0.06, sea_env(p.x));
            let depth = y - p.y;
            if (crest > 0.0 && depth < 0.03 * crest) {
                let fz = noise(vec2<f32>(p.x * 90.0 - F.t * 0.4, depth * 90.0));
                let g = foam_glyph(fz * 1.2, q);
                if (g != 0u) {
                    return pack(g, 6u);
                }
            }
        }
    }
    return 0u;
}

const FUJI_X: f32 = 0.25;
const FUJI_TOP: f32 = -0.05;
const HORIZON: f32 = -0.25;

// Fuji's profile: steep near the summit, spreading at the foot, a flat crater top.
fn fuji_h(x: f32) -> f32 {
    let d = max(abs(x - FUJI_X) - 0.01, 0.0);
    return FUJI_TOP - 0.2 * pow(d / 0.24, 0.72);
}

fn fuji(q: vec2<f32>, p: vec2<f32>) -> u32 {
    let cs = cell_size();
    if (abs(p.x - FUJI_X) > 0.3) {
        return 0u;
    }
    let y = fuji_h(p.x);
    if (y < HORIZON - cs.y) {
        return 0u;
    }
    let slope = (fuji_h(p.x + cs.x * 0.5) - fuji_h(p.x - cs.x * 0.5)) / cs.x;
    let n = normalize(vec2<f32>(-slope, 1.0));
    let g = pen_line((p.y - y) * n.y, n, 0.0);
    if (g != 0u) {
        return pack(g, select(4u, 6u, p.y > FUJI_TOP - 0.07));
    }
    if (p.y > y) {
        return 0u;
    }
    // Snowline: a jagged edge of runs coming down the slopes.
    let dx = p.x - FUJI_X;
    let tooth = abs(fract(dx / 0.03) * 2.0 - 1.0);
    let snow = FUJI_TOP - 0.05 - 0.035 * tooth * tooth - 0.01 * noise(vec2<f32>(dx * 60.0, 3.0));
    let sn = normalize(vec2<f32>(-(0.035 * 2.0 * tooth * sign(fract(dx / 0.03) - 0.5) * 2.0 / 0.03), 1.0));
    let gs = pen_line((p.y - snow) * sn.y, sn, 0.0);
    if (gs != 0u) {
        return pack(gs, 6u);
    }
    if (p.y > snow) {
        return select(0u, pack(C_DOT, 5u), u32(q.x) % 3u == 0u && u32(q.y) % 2u == 0u);
    }
    return 0u;
}

// Spray thrown off the claws, falling slowly like snow toward Fuji.
fn spray(q: vec2<f32>) -> u32 {
    for (var i = 0u; i < 36u; i++) {
        let fi = f32(i);
        let period = 6.0 + 5.0 * hash11(fi * 3.7);
        let ph = fract(F.t / period + hash11(fi * 1.3));
        let life = 0.75;
        if (ph > life) {
            continue;
        }
        let tau = ph * period;
        let j = i % NCLAW;
        let tip = claws[j * CP + CP - 3u].xy;
        let v0 = vec2<f32>(0.02 + 0.05 * hash11(fi * 5.1), 0.01 + 0.03 * hash11(fi * 7.9));
        let pos = tip + v0 * tau + vec2<f32>(0.0, -0.004 * tau * tau);
        let at = floor(to_cells(pos));
        if (all(at == q)) {
            let age = ph / life;
            var g = C_DOT;
            if (age < 0.15) {
                g = C_O;
            } else if (age < 0.4) {
                g = select(C_STAR, C_QUOTE, hash11(fi) > 0.5);
            }
            return pack(g, tone_slot(0.95 - 0.6 * age));
        }
    }
    return 0u;
}

// Long thin clouds drifting left across the upper sky.
fn clouds(q: vec2<f32>, p: vec2<f32>) -> u32 {
    let rows = f32(F.rows);
    for (var i = 0; i < 3; i++) {
        let fi = f32(i);
        let yb = 0.36 - 0.12 * fi;
        let len = 0.5 + 0.2 * hash11(fi + 9.0);
        let span = F.aspect + 2.0 * len;
        let xc = 0.5 * span - fract(0.2 + 0.37 * fi + F.t * (0.0016 + 0.0005 * fi) / span) * span;
        let u = (p.x - xc) / len;
        if (abs(u) > 0.5) {
            continue;
        }
        let j = q.y - floor((0.5 - yb) * rows);
        if (j < 0.0 || j > 1.0) {
            continue;
        }
        let w = 2.0 * (u + 0.1 * j) / (1.0 - 0.35 * j);
        let e = 1.0 - w * w;
        let dens = e * (0.35 + 0.9 * noise(vec2<f32>(u * 9.0 + j * 3.1, fi * 9.1 + j * 2.0)));
        if (dens < 0.3 || (dens < 0.45 && u32(q.x) % 2u == 0u)) {
            continue;
        }
        let g = select(select(C_DASH, C_TILDE, dens > 0.7), C_UNDER, j > 0.0);
        return pack(g, tone_slot(0.12 + 0.2 * dens));
    }
    return 0u;
}

fn cell(c: vec2<u32>) -> u32 {
    let q = vec2<f32>(c);
    let p = to_p(c, vec2<f32>(0.5));
    let cs = cell_size();
    let h = sea_h(p.x);
    if (p.y < h + cs.y) {
        let g = sea(q, p, h);
        if (g != 0u || p.y < h) {
            return g;
        }
    }
    if (p.x > -0.3 && p.x < 0.6 && p.y > -0.4 && p.y < 0.35) {
        let s = spray(q);
        if (s != 0u) {
            return s;
        }
    }
    if (p.x < 0.12 && p.y > -0.5) {
        let g = great_wave(q, p);
        if (g != 0u) {
            return g;
        }
        if (inside_wave(p)) {
            return 0u;
        }
    }
    let f = fuji(q, p);
    if (f != 0u) {
        return f;
    }
    if (abs(p.y - HORIZON) < cs.y * 0.5) {
        return pack(C_UNDER, 2u);
    }
    return clouds(q, p);
}
