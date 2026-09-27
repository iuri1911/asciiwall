// koi — after yuruyurau's #つぶやきProcessing creatures (x.com/yuruyurau): a body
// is nothing but thousands of points on moving curves, drawn translucent so
// density does the shading. Here every point lands in one of a character's 2x3
// samples, and shape matching turns the streams into ~ - _ / \ ( ) strokes.
// Three koi circle a pond (the body follows the swim path like a ribbon, so it
// bends into turns), their spots are highlight-colored, and the water answers
// with rings. Formulas are my own; the idea of drawing creatures as point
// streams is theirs. Each workgroup first walks every swim path back by arc
// length, so a body keeps its length however fast the head is moving.

const FISH: u32 = 3u;
const PER_FISH: u32 = 9080u; // 7 strands + tail 2000 + fins 800 + eyes 400
const STRANDS: u32 = 7u;
const PER_STRAND: u32 = 840u;
const RINGS: u32 = 8u;
const PER_RING: u32 = 700u;
const NODES: u32 = 48u; // spine samples per fish, head first

var<workgroup> spine_pos: array<vec2<f32>, 144>; // FISH * NODES
var<workgroup> spine_dir: array<vec2<f32>, 144>;

fn fish_len(f: u32) -> f32 {
    return 0.24 + 0.04 * f32(f);
}

// Head position of fish `f` at time `tau`: a lap around the pond on a wobbling
// ellipse, wide enough that the tightest turn (radius ~ b^2/a) stays near a
// body length. The middle fish swims the other way.
fn path(f: u32, tau: f32) -> vec2<f32> {
    let ff = f32(f);
    let dir = select(1.0, -1.0, f == 1u);
    let w = dir * tau * (0.11 - 0.012 * ff) + ff * 2.1 + f32(F.seed % 16u);
    let a = (0.3 - 0.035 * ff) * F.aspect;
    let b = 0.3 - 0.03 * ff;
    let wobble = 1.0 + 0.1 * sin(2.0 * w + ff * 1.3);
    let drift = 0.04 * vec2<f32>(sin(tau * 0.013 + ff), cos(tau * 0.017 + ff * 2.0));
    return vec2<f32>(a * cos(w), b * sin(w)) * wobble + drift;
}

// Walk back from the head in equal arc-length steps (midpoint rule on the
// path's speed), one thread per fish.
fn prepare(lid: u32) {
    if (lid >= FISH) {
        return;
    }
    let f = lid;
    let ds = fish_len(f) / f32(NODES - 1u);
    let h = 0.02;
    var tau = F.t;
    for (var n = 0u; n < NODES; n++) {
        let pos = path(f, tau);
        let vel = (pos - path(f, tau - h)) / h;
        let speed = max(length(vel), 1e-4);
        spine_pos[f * NODES + n] = pos;
        spine_dir[f * NODES + n] = vel / speed;
        let mid = tau - 0.5 * ds / speed;
        let vm = length(path(f, mid) - path(f, mid - h)) / h;
        tau -= ds / max(vm, 1e-4);
    }
}

struct Spine {
    pos: vec2<f32>,
    tangent: vec2<f32>,
};

// Spine at u (0 head .. 1 tail), with the swimming wave travelling to the tail.
fn spine(f: u32, u: f32) -> Spine {
    let x = clamp(u, 0.0, 1.0) * f32(NODES - 1u);
    let i = min(u32(x), NODES - 2u);
    let k = f * NODES + i;
    let fr = x - f32(i);
    let pos = mix(spine_pos[k], spine_pos[k + 1u], fr);
    let tangent = normalize(mix(spine_dir[k], spine_dir[k + 1u], fr) + vec2<f32>(1e-6, 0.0));
    let normal = vec2<f32>(-tangent.y, tangent.x);
    let wave = fish_len(f) * 0.05 * pow(u, 1.4) * sin(u * 6.0 - F.t * 4.0 + f32(f));
    return Spine(pos + normal * wave, tangent);
}

fn body_width(f: u32, u: f32) -> f32 {
    return fish_len(f) * 0.14 * pow(sin(PI * min(u * 1.15, 1.0)), 0.6) * (1.0 - 0.65 * u) + 0.002;
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
