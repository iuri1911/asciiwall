// dandelion — a seed head (a "clock") on a thin stem, shedding seeds on the
// wind. The head is a Fibonacci sphere of pappus tufts turning slowly around
// the stem: each tuft is one speck (* +), bright in front and around the rim,
// dim behind, and a few broadside stalks radiate from the receptacle knot (@)
// as fine line work (/ \ | - ' .). Gusts pluck a few seeds at a time; each
// rides a curl-noise wind (after Bridson's curl noise) as a tiny tumbling
// parachute, a tuft over its hand-set stalk, while its slot on the head grows
// back. The stem is hand-set too: one | per row, a \ or / where it steps over.

const HEAD: u32 = 160u;     // seeds on the head
const SPOKES: u32 = 64u;    // most stalks drawn at once
const RELEASE: u32 = 5u;    // at most this many seeds per gust
const KEPT: i32 = 7;        // gusts whose seeds may still be airborne
const POOL: u32 = 35u;      // RELEASE * KEPT drifting seeds
const GUST: f32 = 10.0;     // mean seconds between gusts
const STEM: u32 = 12u;      // stem polyline segments
const STEPS: u32 = 24u;     // flow integration steps per drifting seed
const R: f32 = 0.22;        // head radius, receptacle to tuft
const CORE: f32 = 0.016;    // receptacle radius
const NECK: f32 = 0.9;      // stalks end here (fraction of R); the tuft is beyond

// Head seeds: projected direction xy, depth z (toward the viewer), growth w.
var<workgroup> head: array<vec4<f32>, HEAD>;
// Head tufts: cell center xy, ink, tone; and 1 / speck size.
var<workgroup> tuft: array<vec4<f32>, HEAD>;
var<workgroup> tuft_size: array<f32, HEAD>;
// Visible head stalks: drawn part (start xy, end xy); brightness, tone.
var<workgroup> spoke_seg: array<vec4<f32>, SPOKES>;
var<workgroup> spoke_look: array<vec2<f32>, SPOKES>;
var<workgroup> spokes: atomic<u32>;
// Drifting seeds: tuft xy, stalk vector zw (tuft -> achene).
var<workgroup> flyer: array<vec4<f32>, POOL>;
// Drifting seeds: brightness, alpha, tone, how much stalk shows.
var<workgroup> flyer_look: array<vec4<f32>, POOL>;
var<workgroup> stem: array<vec2<f32>, 13>;
var<workgroup> center: vec2<f32>;

fn gust_time(n: i32) -> f32 {
    return (f32(n) + 0.4 * (hash11(f32(n) * 1.71 + 0.3) - 0.5)) * GUST;
}

fn gust_count(n: i32) -> u32 {
    return 1u + u32(hash11(f32(n) * 3.13 + 1.9) * 4.99);
}

// Wind strength: each gust swells over two seconds, then dies away over ~8 s.
fn gust(t: f32) -> f32 {
    let k = i32(floor(t / GUST));
    var g = 0.0;
    for (var n = k - 1; n <= k + 1; n++) {
        let x = t - gust_time(n);
        g += f32(gust_count(n)) / f32(RELEASE) * smoothstep(-2.5, 0.0, x) * (1.0 - smoothstep(0.0, 8.0, x));
    }
    return g;
}

fn sway(t: f32) -> f32 {
    return 0.008 * sin(0.41 * t) + 0.005 * sin(0.67 * t + 1.3) + 0.02 * gust(t);
}

fn root_x() -> f32 {
    return -F.aspect / 6.0;
}

fn head_center(s: f32) -> vec2<f32> {
    return vec2<f32>(root_x() + 0.05 + s, 0.09 - 2.0 * s * s);
}

// Direction of head seed `i` at time `t` with the stem swayed by `s`,
// projected: xy on screen, z depth.
fn seed_dir(i: u32, t: f32, s: f32) -> vec3<f32> {
    let fi = f32(i);
    let y = mix(0.97, -0.74, (fi + 0.5) / f32(HEAD));
    let r = sqrt(max(1.0 - y * y, 0.0));
    let a = fi * 2.39996323 + 0.25 * (hash11(fi * 5.3) - 0.5) + t * 0.03 + f32(F.seed % 17u);
    var d = vec3<f32>(r * cos(a), y, r * sin(a));
    // Seen from slightly above, and leaning with the stem.
    let cp = cos(0.22);
    let sp = sin(0.22);
    d = vec3<f32>(d.x, d.y * cp - d.z * sp, d.y * sp + d.z * cp);
    let q = rot(-0.05 - 1.6 * s) * d.xy;
    return vec3<f32>(q, d.z);
}

// How much of head seed `i`'s stalk shows (direction `d`), counted from the
// tuft inward: one seed in three, and only while it lies broadside on the
// near half, so the head reads as a clean starburst instead of a thicket.
fn spoke(i: u32, d: vec3<f32>) -> f32 {
    return select(0.0, 1.0, i % 3u == 1u) * smoothstep(0.8, 0.95, length(d.xy)) * smoothstep(-0.25, 0.1, d.z);
}

// Tufts behind the head only show around its rim; inside the disc the near
// half's fluff hides them.
fn seen(d: vec3<f32>) -> f32 {
    return max(smoothstep(-0.3, 0.05, d.z), smoothstep(0.8, 0.95, length(d.xy)));
}

// Brightness of a head tuft: the near side is brighter, and the rim glows,
// where the fluff is seen edge-on, so the clock keeps a round silhouette.
fn glow(d: vec3<f32>) -> f32 {
    let front = 0.5 + 0.5 * d.z;
    return max(mix(0.15, 0.75, front), smoothstep(0.75, 0.98, length(d.xy)) * mix(0.75, 1.0, front));
}

// Divergence-free wind: a steady breeze plus the curl of three travelling waves.
fn wind(x: vec2<f32>, t: f32) -> vec2<f32> {
    let g = gust(t);
    var v = vec2<f32>(0.018 + 0.045 * g, 0.003 + 0.01 * g);
    let k1 = vec2<f32>(5.0, 7.0);
    let k2 = vec2<f32>(-8.0, 4.0);
    let k3 = vec2<f32>(3.0, -9.5);
    v += 0.0015 * cos(dot(k1, x) + 0.21 * t + 1.0) * vec2<f32>(k1.y, -k1.x);
    v += 0.0012 * cos(dot(k2, x) - 0.17 * t + 2.0) * vec2<f32>(k2.y, -k2.x);
    v += 0.001 * cos(dot(k3, x) + 0.13 * t + 4.0) * vec2<f32>(k3.y, -k3.x);
    return v;
}

// Drifting seed `j` belongs to every KEPT-th gust; it is the seed that the
// latest such gust (already blown) plucked, followed from its release.
fn launch(j: u32, t: f32) {
    let cls = i32(j / RELEASE);
    let m = j % RELEASE;
    let k = i32(floor(t / GUST + 0.5));
    var n = k - (((k - cls) % KEPT) + KEPT) % KEPT;
    if (gust_time(n) > t) {
        n -= KEPT;
    }
    let t0 = gust_time(n);
    let age = t - t0;
    if (m >= gust_count(n)) {
        flyer_look[j] = vec4<f32>(0.0);
        return;
    }
    // Consecutive releases land on distinct slots (37 is coprime to HEAD).
    let r = n * i32(RELEASE) + i32(m);
    let slot = u32(((r * 37) % i32(HEAD) + i32(HEAD)) % i32(HEAD));
    head[slot].w = smoothstep(12.0, 26.0, age);

    // At release the seed is exactly the head seed it was.
    let s0 = sway(t0);
    let d0 = seed_dir(slot, t0, s0);
    var x = head_center(s0) + R * d0.xy;
    let h = age / f32(STEPS);
    for (var s = 0u; s < STEPS; s++) {
        let ts = t0 + f32(s) * h;
        x += h * wind(x + 0.5 * h * wind(x, ts), ts + 0.5 * h);
    }

    // Tumbling: from its pose on the head it swings achene-down, then rocks.
    let hr = hash11(f32(r) * 0.917 + 4.0);
    let up = normalize(vec3<f32>(
        0.45 * sin(0.2 * age + hr * 6.0),
        1.0,
        0.8 * sin(0.13 * age + hr * 11.0)
    ));
    let settle = smoothstep(0.0, 4.0, age);
    let axis = normalize(mix(d0, up, smoothstep(0.0, 6.0, age)));
    let len = (R - CORE) * mix(1.0, 0.2 + 0.15 * hr, smoothstep(0.0, 2.5, age));
    let alpha = mix(seen(d0), 1.0, settle) * (1.0 - smoothstep(52.0, 60.0, age));
    var tone = 1.0;
    // A few seeds catch the light now and then.
    if (hr < 0.3 && sin(0.3 * age + hr * 20.0) > 0.55) {
        tone = 1.2;
    }
    // Out of the head's crowd every seed shows its stalk.
    let vis = mix(spoke(slot, d0), smoothstep(0.15, 0.45, length(axis.xy)), settle);
    flyer[j] = vec4<f32>(x, -axis.xy * len);
    flyer_look[j] = vec4<f32>(mix(glow(d0), 0.5 + 0.45 * hr, settle), alpha, tone, vis);
}

// Center of the character cell under `q`: tufts sit in one cell each and hop
// cell to cell, so a speck is always a whole glyph, never split in two.
fn snap(q: vec2<f32>) -> vec2<f32> {
    let n = vec2<f32>(f32(F.cols), f32(F.rows));
    let u = (floor(vec2<f32>(q.x / F.aspect + 0.5, 0.5 - q.y) * n) + 0.5) / n;
    return vec2<f32>((u.x - 0.5) * F.aspect, 0.5 - u.y);
}

fn reach(g: f32) -> f32 {
    return CORE + (R - CORE) * g;
}

fn speck_ink(lit: f32) -> f32 {
    return mix(0.35, 1.0, lit);
}

fn speck_tone(lit: f32, tone: f32) -> f32 {
    return select(mix(0.3, 0.95, lit) * tone, tone, tone > 1.0);
}

fn stalk_ink(lit: f32) -> f32 {
    return mix(0.45, 0.7, lit);
}

fn stalk_tone(lit: f32, tone: f32) -> f32 {
    return mix(0.15, 0.6, lit) * min(tone, 1.0);
}

// Everything per seed is settled here once per workgroup, so each sample only
// measures distances.
fn prepare(lid: u32) {
    let t = F.t;
    let s = sway(t);
    let c = head_center(s);
    if (lid == 0u) {
        center = c;
        atomicStore(&spokes, 0u);
    }
    if (lid <= STEM) {
        let base = vec2<f32>(root_x() - 0.03, -0.56);
        let bend = vec2<f32>(root_x() + 0.08 + 0.5 * s, -0.2);
        let top = c - vec2<f32>(0.0, CORE * 0.7);
        let u = f32(lid) / f32(STEM);
        stem[lid] = (1.0 - u) * (1.0 - u) * base + 2.0 * u * (1.0 - u) * bend + u * u * top;
    }
    for (var i = lid; i < HEAD; i += 64u) {
        head[i] = vec4<f32>(seed_dir(i, t, s), 1.0);
    }
    workgroupBarrier();
    if (lid < POOL) {
        launch(lid, t);
    }
    workgroupBarrier();
    let light = normalize(vec3<f32>(-0.5, 0.55, 0.65));
    for (var i = lid; i < HEAD; i += 64u) {
        let h = head[i];
        let grown = smoothstep(0.6, 1.0, h.w);
        let lit = glow(h.xyz);
        // The tufts facing the light glint in the highlight color.
        let tone = select(1.0, 1.2, dot(h.xyz, light) > 0.93);
        tuft[i] = vec4<f32>(snap(c + reach(h.w) * h.xy), speck_ink(lit) * grown * seen(h.xyz), speck_tone(lit, tone));
        tuft_size[i] = 1.0 / (0.4 + 0.6 * grown);
        let vis = spoke(i, h.xyz);
        if (vis > 0.02 && h.w > 0.0) {
            let k = atomicAdd(&spokes, 1u);
            if (k < SPOKES) {
                let a = c + CORE * h.xy;
                let b = c + min(reach(h.w), R * NECK) * h.xy;
                spoke_seg[k] = vec4<f32>(b - (b - a) * vis, b);
                spoke_look[k] = vec2<f32>(0.5 + 0.5 * h.z, tone);
            }
        }
    }
}

fn keep(best: vec3<f32>, ink: f32, tone: f32, mat: f32) -> vec3<f32> {
    if (ink > best.x) {
        return vec3<f32>(ink, tone, mat);
    }
    return best;
}

// The stem's x at height `y` (the stem rises monotonically).
fn stem_x(y: f32) -> f32 {
    var x = stem[0].x;
    for (var i = 0u; i < STEM; i++) {
        let a = stem[i];
        let b = stem[i + 1u];
        if (y >= a.y && y <= b.y) {
            x = mix(a.x, b.x, (y - a.y) / max(b.y - a.y, 1e-6));
        }
    }
    return x;
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-8), 0.0, 1.0);
    return length(pa - ba * h);
}

// Hand-set ink of a steep line through `p`'s row: it spans heights ya (top)
// to yb (bottom) there, at x = xa and xb. The row gets one glyph, in the
// column the line crosses mid-row: a | or, where the line steps over to the
// next column, a \ or /. So steep lines never smear across two columns.
fn row_mark(p: vec2<f32>, ya: f32, yb: f32, xa: f32, xb: f32) -> f32 {
    let cs = cell_size();
    let cell = snap(p);
    if (ya <= yb || abs(snap(vec2<f32>(0.5 * (xa + xb), cell.y)).x - cell.x) > 0.25 * cs.x) {
        return 0.0;
    }
    var lean = 0.0;
    if (abs(snap(vec2<f32>(xa, cell.y)).x - snap(vec2<f32>(xb, cell.y)).x) > 0.5 * cs.x) {
        lean = sign(xb - xa) * 0.35 * cs.x;
    }
    let a = vec2<f32>(cell.x - lean, min(ya, cell.y + 0.45 * cs.y));
    let b = vec2<f32>(cell.x + lean, max(yb, cell.y - 0.45 * cs.y));
    let dir = normalize(b - a + vec2<f32>(0.0, -1e-6));
    return stroke(seg_dist(p, a, b), vec2<f32>(-dir.y, dir.x));
}

// Ink of a stalk drawn from `a` to `b`: steep stalks are hand-set row by
// row, the rest are plain (slightly narrowed) strokes.
fn stalk(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    if (dot(ab, ab) < 1e-8) {
        return 0.0;
    }
    let cs = cell_size();
    if (abs(ab.y) / cs.y >= abs(ab.x) / cs.x) {
        let hi = select(a, b, b.y > a.y);
        let lo = select(b, a, b.y > a.y);
        let cell = snap(p);
        let ya = min(cell.y + 0.5 * cs.y, hi.y);
        let yb = max(cell.y - 0.5 * cs.y, lo.y);
        let k = (hi.x - lo.x) / max(hi.y - lo.y, 1e-6);
        return row_mark(p, ya, yb, lo.x + (ya - lo.y) * k, lo.x + (yb - lo.y) * k);
    }
    let dir = normalize(ab);
    return stroke(seg_dist(p, a, b) * 1.25, vec2<f32>(-dir.y, dir.x));
}

// A tuft speck centered in its cell, `inv_size` = 1 / its size.
fn speck(p: vec2<f32>, at: vec2<f32>, inv_size: f32) -> f32 {
    return smoothstep(0.62, 0.3, length((p - at) / cell_size()) * inv_size);
}

fn field(p0: vec2<f32>) -> vec3<f32> {
    var p = p0;
    if ((F.seed & 1u) == 1u) {
        p.x = -p.x;
    }
    let cs = cell_size();
    var best = vec3<f32>(0.0);

    // Stem, hand-set like the steep stalks.
    let top = stem[STEM];
    if (p.y < top.y + cs.y && abs(p.x - root_x() - 0.03) < 0.16) {
        let cell = snap(p);
        let ya = min(cell.y + 0.5 * cs.y, top.y);
        let yb = cell.y - 0.5 * cs.y;
        best = keep(best, row_mark(p, ya, yb, stem_x(ya), stem_x(yb)) * 0.8, 0.3, 3.0);
    }

    // Head.
    let c = center;
    let rv = length(p - c);
    if (rv < R + 0.03) {
        // The receptacle: a knot a few characters wide, one row tall.
        let knot = smoothstep(1.0, 0.55, length((p - snap(c)) / cs / vec2<f32>(2.3, 0.6)));
        best = keep(best, knot * 0.9, 0.5, 2.0);
        for (var i = 0u; i < HEAD; i++) {
            let f = tuft[i];
            let d = (p - f.xy) / cs;
            if (f.z > 0.0 && dot(d, d) < 0.5) {
                best = keep(best, speck(p, f.xy, tuft_size[i]) * f.z, f.w, 1.0);
            }
        }
        // Stalks emerge from under the knot.
        let emerge = smoothstep(CORE, CORE * 2.5, rv);
        let n = min(atomicLoad(&spokes), SPOKES);
        for (var k = 0u; k < n; k++) {
            let g = spoke_seg[k];
            let look = spoke_look[k];
            best = keep(best, stalk(p, g.xy, g.zw) * stalk_ink(look.x) * emerge, stalk_tone(look.x, look.y), 0.0);
        }
    }

    // Seeds on the wind.
    for (var j = 0u; j < POOL; j++) {
        let look = flyer_look[j];
        if (look.y <= 0.0) {
            continue;
        }
        let f = flyer[j];
        if (distance(p, f.xy) > length(f.zw) + 0.02) {
            continue;
        }
        let tip = snap(f.xy);
        best = keep(best, speck(p, tip, 1.0) * speck_ink(look.x) * look.y, speck_tone(look.x, look.z), 1.0);
        if (look.w > 0.02) {
            // The stalk hangs from the tuft's neck down to the achene.
            let neck = f.xy + f.zw * (1.0 - NECK) * R / (R - CORE);
            let achene = f.xy + f.zw;
            let s = stalk(p, neck + (achene - neck) * look.w, neck);
            best = keep(best, s * stalk_ink(look.x) * look.y, stalk_tone(look.x, look.z), 0.0);
        }
    }
    return best;
}
