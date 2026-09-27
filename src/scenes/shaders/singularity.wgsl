// singularity — after XorDev's black holes, Gargantua as the picture: a true
// empty shadow, one hot photon ring, and an accretion disk that is streaks
// rather than a bar of text. The approaching limb is bright and continuous;
// the receding limb breaks into .-~ . A jet of rising sparks and a few embers
// in orbit keep it from sitting still. Far-side light is lensed into arcs
// over the hole, drawn with ( on the left and ) on the right.

const CENTER: vec2<f32> = vec2<f32>(0.0, -0.02);
const RS: f32 = 0.20;
const ROLL: f32 = 0.04;
const K: f32 = 0.22;
const RIN: f32 = 1.15;
const ROUT: f32 = 3.6;
const RING: f32 = 1.04;
const ARCW: f32 = 0.72;
const LANES: f32 = 2.2;
const SPIN: f32 = 0.22;

fn orbit_noise(x: f32, n: f32, lane: f32) -> f32 {
    let i = floor(x);
    let f = x - i;
    let j = i + 1.0;
    let a = hash21(vec2<f32>(lane, i - n * floor(i / n)));
    let b = hash21(vec2<f32>(lane, j - n * floor(j / n)));
    return mix(a, b, f * f * (3.0 - 2.0 * f));
}

fn lane(l: f32, psi: f32) -> f32 {
    let rho = RIN + (l + 0.5) / LANES;
    let turns = (psi - SPIN * pow(RIN / rho, 1.5) * F.t) / TAU;
    let n = max(round(rho * 5.0), 5.0);
    return 0.65 * orbit_noise(turns * n, n, l) + 0.35 * orbit_noise(turns * n * 2.0, n * 2.0, l + 19.0);
}

// (ink, tone). Streaks, not a filled band. `solid` only thickens the hot limb.
fn shade(rho: f32, psi: f32, tex: f32, solid: f32) -> vec2<f32> {
    let u = (rho - RIN) / (ROUT - RIN);
    let streak = smoothstep(0.55, 0.82, tex);
    let heat = pow(RIN / rho, 2.4);
    // Left side approaches. Beam it hard and let the right side go quiet.
    let dop = 1.0 - 0.72 * sqrt(RIN / rho) * cos(psi);
    let beam = dop * dop * dop;
    let edge = smoothstep(-0.02, 0.03, u) * (1.0 - smoothstep(0.08, 0.85, u));
    let fill = solid * clamp(beam, 0.0, 1.2) * (1.0 - u);
    let ink = min(1.0, edge * mix(streak, 0.85, fill) * (0.25 + 0.85 * min(beam, 1.4)));
    let tone = 0.12 + heat * min(beam, 1.6) * (0.45 + 0.55 * streak);
    return vec2<f32>(ink, tone);
}

fn disk(rho: f32, psi: f32, solid: f32) -> vec2<f32> {
    if (rho < RIN - 0.08 || rho > ROUT) {
        return vec2<f32>(0.0);
    }
    let x = (rho - RIN) * LANES - 0.5;
    let l = floor(x);
    let tex = mix(lane(l, psi), lane(l + 1.0, psi), smoothstep(0.2, 0.8, x - l));
    return shade(rho, psi, tex, solid);
}

fn stars(q: vec2<f32>, r: f32) -> vec3<f32> {
    let sp = q * (1.0 - 0.85 / (r * r)) * RS;
    let cs = cell_size();
    let g = cs * vec2<f32>(5.0, 3.0);
    let id = floor(sp / g);
    let h = hash21(id);
    if (h < 0.94) {
        return vec3<f32>(0.0);
    }
    let c = (id + 0.3 + 0.4 * vec2<f32>(hash21(id + 3.1), hash21(id + 7.7))) * g;
    let e = length((sp - c) / cs * vec2<f32>(1.0, 1.5));
    let tw = 0.6 + 0.4 * sin(F.t * 0.8 + h * 60.0);
    return vec3<f32>(0.8 * tw * smoothstep(0.4, 0.15, e), select(0.3, 1.2, h > 0.985), 2.0);
}

// Rising sparks in a narrow jet, above and below the shadow.
fn jet(q: vec2<f32>) -> vec3<f32> {
    let ay = abs(q.y);
    if (ay < 1.05 || ay > 2.6) {
        return vec3<f32>(0.0);
    }
    let rise = fract(ay * 1.7 - F.t * 0.18 * sign(q.y));
    let w = 0.035 + 0.05 * (ay - 1.0);
    let column = stroke(abs(q.x) * RS, vec2<f32>(1.0, 0.0));
    let spark = smoothstep(0.28, 0.0, abs(rise - 0.45));
    let v = max(column * 0.7, spark * smoothstep(0.12, 0.02, abs(q.x))) * exp(-(ay - 1.05) * 0.4);
    if (v < 0.05) {
        return vec3<f32>(0.0);
    }
    return vec3<f32>(min(v * 1.4, 1.0), select(0.55, 1.4, spark > 0.3), 1.0);
}

fn embers(q: vec2<f32>) -> vec3<f32> {
    var best = vec3<f32>(0.0);
    let cs = cell_size() / RS;
    for (var i = 0; i < 6; i++) {
        let fi = f32(i);
        let rho = 1.45 + fi * 0.28;
        let ang = F.t * (0.35 / pow(rho, 1.5)) + fi * 1.7 + hash11(fi) * TAU;
        let c = vec2<f32>(cos(ang), sin(ang) * K) * rho;
        let e = length((q - c) / cs);
        let v = smoothstep(0.55, 0.12, e);
        if (v > best.x) {
            best = vec3<f32>(v, 1.45, 2.0);
        }
    }
    return best;
}

fn field(p0: vec2<f32>) -> vec3<f32> {
    var p = p0 - CENTER;
    let arc = select(3.0, 1.0, p.x < 0.0);
    if ((F.seed & 1u) == 1u) {
        p.x = -p.x;
    }
    let q = rot(ROLL) * p / RS;
    let r = length(q);
    let ez = q.y / K;
    let rn = length(vec2<f32>(q.x, ez));

    // Near side of the disk, in front of the shadow. Streaks, hot on the left.
    if (q.y < -0.02 && rn > RIN * 0.98 && rn < ROUT) {
        let d = disk(rn, atan2(ez, q.x), 0.0);
        if (d.x > 0.08) {
            return vec3<f32>(min(d.x, 0.72), d.y, 0.0);
        }
    }

    var best = vec3<f32>(0.0, 0.0, 2.0);
    if (r < 0.98) {
        return best;
    }
    if (r > 1.8) {
        best = stars(q, r);
    }

    let phi = atan2(q.y, q.x);
    let n = rot(-ROLL) * (q / r);
    let row = cell_size().y / RS;

    if (q.y > 0.02) {
        let d = disk(rn, atan2(ez, q.x), 0.05);
        if (d.x > best.x) {
            best = vec3<f32>(d.x, d.y, 0.0);
        }
    }

    // Lensed far side: two clean arcs, packed against the shadow, not a hatch.
    let a0 = RING + 1.3 * row;
    if (q.y > 0.15 && r > a0 - row && r < a0 + ARCW) {
        for (var k = 0.0; k < 3.0; k += 1.0) {
            let rl = a0 + row * (1.4 * k + 0.55 * k * k);
            let s = (rl - a0) / ARCW;
            let taper = smoothstep(0.0, 0.18, sin(phi) - 0.15 - 0.55 * s);
            let d = disk(RIN + s * (ROUT - RIN) * 0.8, phi, 0.35);
            let v = min(1.0, d.x * taper * stroke(abs(r - rl) * RS, n) * 1.5);
            if (v > best.x) {
                best = vec3<f32>(v, max(d.y, 0.7 * (1.0 - s)), arc);
            }
        }
    }
    // A thin answering arc under the hole.
    let rb = RING + 1.8 * row;
    if (q.y < -0.05 && abs(r - rb) < 2.2 * row && abs(phi) > 2.2) {
        let d = disk(RIN + 0.3, phi + PI, 0.15);
        let v = d.x * stroke(abs(r - rb) * RS, n);
        if (v > best.x) {
            best = vec3<f32>(v, d.y, arc);
        }
    }

    // Photon ring: one bright circle, hotter where the disk approaches.
    let dop = 1.0 - 0.55 * n.x;
    let ring = max(stroke(abs(r - RING) * RS, n), 0.55 * stroke(abs(r - RING - 0.05) * RS, n)) * (0.75 + 0.4 * dop);
    if (ring > best.x) {
        best = vec3<f32>(min(ring, 1.0), 1.2 + 0.3 * dop, arc);
    }

    let j = jet(q);
    if (j.x > best.x) {
        best = j;
    }
    let e = embers(q);
    if (e.x > best.x && r > 1.15) {
        best = e;
    }
    return best;
}
