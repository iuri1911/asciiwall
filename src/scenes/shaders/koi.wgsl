// koi — after yuruyurau's #つぶやきProcessing creatures (x.com/yuruyurau): a body
// is nothing but thousands of points on moving curves, drawn translucent so
// density does the shading. Here every point lands in one of a character's 2x3
// samples, and shape matching turns the streams into ~ - _ / \ ( ) strokes.
// Three koi circle a pond (the body follows the swim path like a ribbon, so it
// bends into turns), their spots are highlight-colored, and the water answers
// with rings. Formulas are my own; the idea of drawing creatures as point
// streams is theirs.

const FISH: u32 = 3u;
const PER_FISH: u32 = 10900u; // 7 strands + tail 2000 + fins 800 + eyes 400
const STRANDS: u32 = 7u;
const PER_STRAND: u32 = 1100u;
const RINGS: u32 = 8u;
const PER_RING: u32 = 700u;

fn fish_len(f: u32) -> f32 {
    return 0.3 + 0.05 * f32(f);
}

// Head position of fish `f` at time `tau`: a slow figure-eight around the pond.
fn path(f: u32, tau: f32) -> vec2<f32> {
    let ff = f32(f);
    let w = tau * (0.075 - 0.01 * ff) + ff * 2.1;
    return vec2<f32>(
        (0.33 - 0.05 * ff) * F.aspect * sin(w),
        (0.24 - 0.04 * ff) * sin(2.0 * w + ff) + 0.05 * (ff - 1.0)
    );
}

struct Spine {
    pos: vec2<f32>,
    tangent: vec2<f32>,
};

// Spine at u (0 head .. 1 tail): the body trails along its own path.
fn spine(f: u32, u: f32) -> Spine {
    // Time lag that puts the tail one body length back along the path.
    let speed = length(path(f, F.t) - path(f, F.t - 0.1)) / 0.1;
    let lag = u * fish_len(f) / max(speed, 1e-3);
    let a = path(f, F.t - lag);
    let b = path(f, F.t - lag - 0.05);
    let tangent = normalize(a - b + vec2<f32>(1e-6, 0.0));
    let normal = vec2<f32>(-tangent.y, tangent.x);
    let wave = fish_len(f) * 0.05 * pow(u, 1.4) * sin(u * 6.0 - F.t * 4.0 + f32(f));
    return Spine(a + normal * wave, tangent);
}

fn body_width(f: u32, u: f32) -> f32 {
    return fish_len(f) * 0.11 * pow(sin(PI * min(u * 1.15, 1.0)), 0.6) * (1.0 - 0.65 * u) + 0.002;
}

fn fish_point(f: u32, j: u32) {
    let len = fish_len(f);
    let base_tone = 0.55 + 0.15 * f32(f);
    if (j < STRANDS * PER_STRAND) {
        let v = f32(j / PER_STRAND) / f32(STRANDS - 1u) * 2.0 - 1.0;
        let u = f32(j % PER_STRAND) / f32(PER_STRAND);
        let s = spine(f, u);
        let n = vec2<f32>(-s.tangent.y, s.tangent.x);
        let spots = noise(vec2<f32>(u * 7.0 + f32(f) * 13.0, v * 1.6 + f32(f) * 5.0));
        let tone = select(base_tone + 0.2 * (1.0 - abs(v)), 1.2, spots > 0.62 && u > 0.1 && u < 0.85);
        plot(s.pos + n * v * body_width(f, u), tone);
        return;
    }
    var k = j - STRANDS * PER_STRAND;
    if (k < 2000u) {
        // Tail fin: rays fanning back from the tail, flapping with the body wave.
        let ray = f32(k / 200u) / 9.0 * 2.0 - 1.0;
        let s = f32(k % 200u) / 200.0;
        let end = spine(f, 1.0);
        let flap = 0.35 * sin(7.0 - F.t * 4.5 + f32(f));
        let a = atan2(-end.tangent.y, -end.tangent.x) + ray * 0.55 * (0.4 + s) + flap * s;
        plot(end.pos + vec2<f32>(cos(a), sin(a)) * s * len * 0.32, base_tone + 0.1);
        return;
    }
    k -= 2000u;
    if (k < 800u) {
        // Pectoral fins: two small fans sweeping back from the chest.
        let side = select(-1.0, 1.0, k < 400u);
        let kk = k % 400u;
        let ray = f32(kk / 80u) / 4.0;
        let s = f32(kk % 80u) / 80.0;
        let chest = spine(f, 0.22);
        let n = vec2<f32>(-chest.tangent.y, chest.tangent.x) * side;
        let dir = normalize(n * (0.9 - 0.5 * ray) - chest.tangent * (0.6 + 0.6 * ray));
        let beat = 0.3 * sin(F.t * 2.3 + side);
        let d = normalize(dir + n * beat * s);
        plot(chest.pos + n * body_width(f, 0.22) + d * s * len * 0.16, base_tone);
        return;
    }
    k -= 800u;
    // Eyes: two tight rings near the head (tone > 2 marks them as eyes).
    let side = select(-1.0, 1.0, k < 200u);
    let a = f32(k % 200u) / 200.0 * TAU;
    let head = spine(f, 0.07);
    let n = vec2<f32>(-head.tangent.y, head.tangent.x);
    plot(head.pos + n * side * body_width(f, 0.07) * 0.55 + vec2<f32>(cos(a), sin(a)) * len * 0.012, 2.5);
}

fn ring_point(k: u32) {
    let r = k / PER_RING;
    let phase = fract(F.t * 0.08 + f32(r) / f32(RINGS));
    // Fewer points as a ring spreads, so it fades instead of thinning into dots.
    if (hash11(f32(k) * 0.37) > 1.0 - phase) {
        return;
    }
    let cycle = floor(F.t * 0.08 + f32(r) / f32(RINGS));
    let c = (vec2<f32>(hash11(f32(r) + cycle * 7.3), hash11(f32(r) * 3.1 + cycle)) - 0.5) * vec2<f32>(F.aspect * 0.9, 0.8);
    let a = f32(k % PER_RING) / f32(PER_RING) * TAU;
    plot(c + vec2<f32>(cos(a), 0.45 * sin(a)) * (0.015 + phase * 0.13), 0.3);
}

fn points(i: u32) {
    if (i < FISH * PER_FISH) {
        fish_point(i / PER_FISH, i % PER_FISH);
    } else {
        ring_point(i - FISH * PER_FISH);
    }
}

fn field(p: vec2<f32>) -> vec3<f32> {
    let s = splat(p);
    let ink = (1.0 - exp(-s.x * 0.18)) * 0.8;
    return vec3<f32>(ink, s.y, select(0.0, 1.0, s.y > 2.0));
}
