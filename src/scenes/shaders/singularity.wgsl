// singularity — after XorDev's black holes: an accretion disk seen nearly
// edge-on, its far side lensed into a halo over the shadow. In ASCII the disk
// is a set of orbits drawn as ellipses of - _ ( ) / \, with hot clumps sliding
// along them (inner orbits faster); the shadow is true emptiness, the photon
// ring is the one line of highlight characters, and the stars behind are bent.

const SHADOW: f32 = 0.15;
const TILT: f32 = 0.26;
const RINGS: i32 = 7;

// Brightness of orbit `i` at angle `a`: clumps orbiting at Keplerian speed.
fn clumps(i: i32, r: f32, a: f32) -> f32 {
    let fi = f32(i);
    let w = a + F.t * 0.22 * pow(0.25 / r, 1.5) + hash11(fi) * TAU;
    let k = 2.0 + f32(i % 3);
    return 0.35 + 0.65 * pow(0.5 + 0.5 * sin(k * w), 3.0);
}

fn ring_radius(i: i32) -> f32 {
    return 0.24 + 0.075 * f32(i);
}

fn field(p0: vec2<f32>) -> vec3<f32> {
    let p = rot(-0.14) * p0;
    let rr = length(p);
    let radial = p / max(rr, 1e-4);
    let doppler = 1.0 - 0.45 * radial.x;
    let cs = cell_size();
    var best = vec3<f32>(0.0, 0.0, 2.0);

    // Stars behind, pulled outward around the shadow.
    let sp = p * (1.0 + 2.0 * SHADOW * SHADOW / max(dot(p, p), 1e-4));
    let id = floor(sp / (cs * vec2<f32>(3.0, 2.0)));
    let h = hash21(id);
    if (h > 0.93 && rr > SHADOW * 1.1) {
        let c = (id + 0.5) * cs * vec2<f32>(3.0, 2.0);
        let tw = 0.55 + 0.45 * sin(F.t * 1.3 + h * 70.0);
        let dd = abs(sp - c) / cs;
        best = vec3<f32>(select(0.0, 0.7 * tw, dd.x < 0.3 && dd.y < 0.2), 0.3 + 0.6 * h, 2.0);
    }

    // Orbits: ellipses in screen space; far halves hide behind the shadow.
    let q = vec2<f32>(p.x, p.y / TILT);
    let r = length(q);
    let a = atan2(q.y, q.x);
    let grad = vec2<f32>(q.x, q.y / TILT) / max(r, 1e-4);
    let n = grad / max(length(grad), 1e-4);
    let occluded = rr < SHADOW && p.y > 0.0;
    if (!occluded) {
        for (var i = 0; i < RINGS; i++) {
            let ri = ring_radius(i);
            let d = abs(r - ri) / max(length(grad), 1e-4);
            let v = stroke(d, n) * clumps(i, ri, a) * doppler * (1.05 - 0.1 * f32(i));
            if (v > best.x) {
                best = vec3<f32>(v, 0.3 + 0.7 * v, select(0.0, 1.0, v > 0.85));
            }
        }
    }

    // The far side of the disk, lensed into arcs hugging the top of the shadow.
    if (rr > SHADOW && p.y > -0.3 * SHADOW) {
        let rl = 0.24 + (rr - SHADOW * 1.08) * 3.2;
        let fade = smoothstep(SHADOW * 2.2, SHADOW * 1.1, rr);
        for (var i = 0; i < 4; i++) {
            let ri = ring_radius(i);
            let d = abs(rl - ri) / 3.2;
            let v = stroke(d, radial) * clumps(i, ri, atan2(p.y, p.x) + PI) * fade * doppler;
            if (v > best.x) {
                best = vec3<f32>(v, 0.25 + 0.6 * v, 0.0);
            }
        }
    }

    // Photon ring.
    let ring = stroke(abs(rr - SHADOW * 1.04), radial) * (0.8 + 0.2 * doppler);
    if (ring > best.x) {
        best = vec3<f32>(ring, 1.3, 0.0);
    }
    return best;
}
