// medusa — also after yuruyurau (x.com/yuruyurau), whose point-stream creatures
// drift like sea life: a jellyfish built from ~30k points on curves. The bell is
// meridians and rings that pulse, tentacles and frilled oral arms trail and lag
// behind each pulse, and marine snow sinks past. Every stream is thin, so shape
// matching draws it as ASCII line work: ( ) / \ | ~ for the ribs and trails.

const MERIDIANS: u32 = 22u;
const PER_MERIDIAN: u32 = 300u;
const RINGS: u32 = 4u;
const PER_RING: u32 = 500u;
const TENTACLES: u32 = 14u;
const PER_TENTACLE: u32 = 600u;
const ARMS: u32 = 4u;
const PER_ARM: u32 = 1800u;
const SNOW: u32 = 1500u;

// Bell contraction: a quick squeeze and a slow release, about every 4 s.
fn pulse(delay: f32) -> f32 {
    let x = fract((F.t - delay) * 0.24);
    return select(1.0 - x / 0.25, (x - 0.25) / 0.75, x > 0.25);
}

fn center() -> vec2<f32> {
    let rise = fract(F.t * 0.004 + 0.5);
    return vec2<f32>(0.22 * F.aspect * sin(F.t * 0.021), 1.4 * rise - 0.55 + 0.015 * pulse(0.0));
}

// Bell surface point: phi from the top (0) to the rim (1), theta around.
fn bell(phi: f32, theta: f32) -> vec2<f32> {
    let squeeze = 1.0 - 0.14 * (1.0 - pulse(phi * 0.3));
    let r = 0.17 * sin(phi * 1.45) * squeeze * (1.0 + 0.12 * phi * phi);
    let h = 0.085 * cos(phi * 1.45) / squeeze;
    let x = r * cos(theta);
    let z = r * sin(theta);
    return center() + vec2<f32>(x, h - 0.03 + 0.2 * z);
}

fn points(i: u32) {
    var k = i;
    let spin = F.t * 0.05;
    if (k < MERIDIANS * PER_MERIDIAN) {
        let m = k / PER_MERIDIAN;
        let phi = f32(k % PER_MERIDIAN) / f32(PER_MERIDIAN);
        let theta = f32(m) / f32(MERIDIANS) * TAU + spin;
        let front = sin(theta) < 0.0;
        plot(bell(phi, theta), select(0.45, 0.85, front));
        return;
    }
    k -= MERIDIANS * PER_MERIDIAN;
    if (k < RINGS * PER_RING) {
        let ring = k / PER_RING;
        let phi = 0.35 + 0.65 * f32(ring) / f32(RINGS - 1u);
        let theta = f32(k % PER_RING) / f32(PER_RING) * TAU;
        plot(bell(phi, theta), select(0.7, 1.3, ring == RINGS - 1u));
        return;
    }
    k -= RINGS * PER_RING;
    if (k < TENTACLES * PER_TENTACLE) {
        let n = k / PER_TENTACLE;
        let s = f32(k % PER_TENTACLE) / f32(PER_TENTACLE);
        let theta = f32(n) / f32(TENTACLES) * TAU + spin + 0.1;
        let root = bell(1.0, theta);
        let len = 0.45 + 0.2 * hash11(f32(n));
        // Each point along a tentacle feels the pulse later, so waves run down it.
        let sway = (0.012 + 0.03 * s) * sin(s * 7.0 - F.t * 1.3 + f32(n) * 1.7) - 0.03 * s * (pulse(s * 1.5) - 0.5);
        plot(root + vec2<f32>(sway, -s * len * (0.9 + 0.1 * pulse(s))), 0.35 + 0.25 * (1.0 - s));
        return;
    }
    k -= TENTACLES * PER_TENTACLE;
    if (k < ARMS * PER_ARM) {
        let a = k / PER_ARM;
        let kk = k % PER_ARM;
        let strand = f32(kk % 3u) - 1.0;
        let s = f32(kk / 3u) / f32(PER_ARM / 3u);
        let fa = f32(a);
        let x0 = (fa / f32(ARMS - 1u) - 0.5) * 0.06;
        let curl = 0.02 * sin(s * 5.0 - F.t * 0.9 + fa * 2.0);
        let frill = (0.006 + 0.008 * s) * sin(s * 55.0 + F.t * 2.0 + fa) * strand;
        plot(center() + vec2<f32>(x0 + curl + frill + strand * 0.004, -0.02 - s * 0.24), 0.7);
        return;
    }
    k -= ARMS * PER_ARM;
    // Marine snow: specks sinking slowly across the whole screen.
    let h = vec2<f32>(hash11(f32(k) * 1.31), hash11(f32(k) * 7.77 + 3.0));
    let y = fract(h.y - F.t * (0.004 + 0.006 * h.x));
    plot(vec2<f32>((h.x - 0.5) * F.aspect + 0.01 * sin(F.t * 0.3 + h.y * 20.0), 0.5 - y), 0.25);
}

fn field(p: vec2<f32>) -> vec3<f32> {
    let s = splat(p);
    let ink = (1.0 - exp(-s.x * 0.22)) * 0.8;
    return vec3<f32>(ink, s.y, 0.0);
}
