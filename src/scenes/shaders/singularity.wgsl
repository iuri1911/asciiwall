// singularity — Gargantua from *Interstellar* as the picture, XorDev's black
// holes for the idea of drawing one from a few analytic layers; formulas are
// ours. Seen almost edge-on: an empty shadow, a thin photon ring, the near side
// of the accretion disk crossing in front of the shadow, and the far side lensed
// into a halo over the top (and a thinner one under the bottom) that tapers into
// the disk at both sides. Gas is drawn the way saturn draws its planet: cells
// wholly inside it take a density ramp, edge cells are shape-matched to line
// glyphs. Keplerian streaks orbit faster inward; the approaching (left) side is
// Doppler-beamed, the receding side thins out to dots.

const CENTER: vec2<f32> = vec2<f32>(0.0, 0.01);
const RS: f32 = 0.2; // shadow radius, screen heights; everything below is in shadow radii
const ROLL: f32 = 0.05; // disk tilt on screen
const K: f32 = 0.17; // projected disk aspect (sine of the viewing angle)
const RIN: f32 = 1.35; // disk inner edge
const ROUT: f32 = 3.8;
const RING: f32 = 1.04; // photon ring
const HALO: f32 = 0.55; // upper halo thickness at its crown
const LANES: f32 = 2.2; // streak lanes per shadow radius
const SPIN: f32 = 0.3; // orbital speed at the inner edge, radians per second
const RAMP = array<u32, 10>(32u, 46u, 45u, 126u, 61u, 43u, 42u, 35u, 37u, 64u); //  .-~=+*#%@

// Sample kinds: sky (stars may show), gas (ramp when a cell is all gas), hole.
const SKY: f32 = 0.0;
const GAS: f32 = 1.0;
const HOLE: f32 = 2.0;

// Value noise around a closed orbit of `n` cells, so it wraps without a seam.
fn loop_noise(x: f32, n: f32, lane: f32) -> f32 {
    let i = floor(x);
    let f = x - i;
    let j = i + 1.0;
    let a = hash21(vec2<f32>(lane, i - n * floor(i / n)));
    let b = hash21(vec2<f32>(lane, j - n * floor(j / n)));
    return mix(a, b, f * f * (3.0 - 2.0 * f));
}

// One lane of gas: clumps along the orbit, drifting at the lane's Kepler rate.
fn lane(l: f32, psi: f32) -> f32 {
    let rho = RIN + (l + 0.5) / LANES;
    let turns = (psi - SPIN * pow(RIN / rho, 1.5) * F.t) / TAU;
    let n = max(round(rho * 4.0), 4.0);
    return 0.65 * loop_noise(turns * n, n, l) + 0.35 * loop_noise(turns * n * 2.0, n * 2.0, l + 19.0);
}

// Lanes blend smoothly across radius: in front the radius is foreshortened to
// a fraction of a row, so only variation along the orbit may be sharp.
fn streaks(rho: f32, psi: f32) -> f32 {
    let x = max(rho - RIN, 0.0) * LANES;
    let l = floor(x);
    return mix(lane(l, psi), lane(l + 1.0, psi), smoothstep(0.0, 1.0, x - l));
}

// Glow of the gas at disk radius `rho`, azimuth `psi` (0 right, pi/2 far side,
// pi left, -pi/2 near side; it orbits toward increasing psi, so the left side
// comes at us). (ink, tone); tone > 1 = highlight.
fn glow(rho: f32, psi: f32) -> vec2<f32> {
    let inner = smoothstep(RIN - 0.06, RIN + 0.12, rho);
    let outer = 1.0 - smoothstep(1.9, ROUT, rho);
    let heat = pow(RIN / rho, 1.5);
    let dop = 1.0 - 0.3 * sqrt(RIN / rho) * cos(psi);
    let e = inner * outer * heat * dop * dop * (0.35 + 0.8 * streaks(rho, psi));
    var tone = 0.1 + 0.75 * min(e, 1.0);
    if (e > 1.35 && rho < RIN + 0.5) {
        tone = 1.2;
    }
    return vec2<f32>(clamp(1.1 * e, 0.0, 1.0), tone);
}

// One sample: (ink, tone, material, kind). Materials: 0 disk, 1 halo, 2 ring.
fn sample(p0: vec2<f32>) -> vec4<f32> {
    var p = p0 - CENTER;
    if ((F.seed & 1u) == 1u) {
        p.x = -p.x;
    }
    let q = rot(ROLL) * p / RS;
    let r = length(q);
    // Undo the foreshortening to get disk-plane polar coordinates.
    let ez = q.y / K;
    let rho = length(vec2<f32>(q.x, ez));
    let psi = atan2(ez, q.x);
    let in_disk = rho > RIN - 0.06 && rho < ROUT;

    // The near side of the disk is in front of everything, the shadow included.
    if (q.y < 0.0 && in_disk) {
        let g = glow(rho, psi);
        if (g.x > 0.04 || r < 1.0) {
            return vec4<f32>(g, 0.0, GAS);
        }
    }
    if (r < 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, HOLE);
    }
    var best = vec4<f32>(0.0, 0.0, 0.0, SKY);
    // The far side seen directly, beside the shadow.
    if (q.y >= 0.0 && in_disk) {
        best = vec4<f32>(glow(rho, psi), 0.0, GAS);
    }
    // The far side again, lensed over the shadow (and, dimmer and thinner, under
    // it). Radius in the halo maps to disk radius; angle around the hole maps to
    // azimuth, so the halo streams the same way as the far side.
    let th = atan2(q.y, q.x);
    let up = q.y > 0.0;
    let w = select(0.28, 1.0, up) * HALO * (0.25 + 0.75 * pow(abs(sin(th)), 0.7));
    let s = (r - RING - 0.05) / w;
    if (s > 0.0 && s < 1.0) {
        let g = glow(RIN + s * (ROUT - RIN) * 0.55, abs(th));
        let ink = g.x * select(0.7, 0.95, up) * smoothstep(1.0, 0.6, s);
        if (ink > best.x) {
            best = vec4<f32>(ink, g.y, 1.0, GAS);
        }
    }
    // Photon ring: one thin circle, hotter on the approaching side.
    let n = rot(-ROLL) * (q / r);
    let dop = 1.0 - 0.5 * q.x / r;
    let ring = stroke(abs(r - RING) * RS, n) * (0.55 + 0.3 * dop);
    if (ring > best.x) {
        best = vec4<f32>(min(ring, 1.0), select(0.75, 1.2, dop > 1.3), 2.0, SKY);
    }
    return best;
}

// Still stars on a scene-space lattice, pulled toward the hole a little
// (lensing), never inside the halo.
fn star(p0: vec2<f32>) -> u32 {
    var p = p0 - CENTER;
    if ((F.seed & 1u) == 1u) {
        p.x = -p.x;
    }
    let r = length(p) / RS;
    if (r < 2.2) {
        return 0u;
    }
    let warp = 1.0 - 0.9 / (r * r);
    let sp = p * warp;
    let cs = cell_size() * warp;
    let lat = vec2<f32>(0.045, 0.06);
    let id = floor(sp / lat);
    let h = hash21(id);
    if (h < 0.86) {
        return 0u;
    }
    let c = (id + 0.5 + 0.6 * (vec2<f32>(hash21(id + 3.7), hash21(id + 9.1)) - 0.5)) * lat;
    let d = abs(c - sp);
    if (d.x > 0.5 * cs.x || d.y > 0.5 * cs.y) {
        return 0u;
    }
    let b = hash21(id + 5.3);
    var ch = 46u; // .
    if (b > 0.94) {
        ch = 42u; // *
    } else if (b > 0.82) {
        ch = 43u; // +
    } else if (b > 0.6) {
        ch = 39u; // '
    }
    return pack(ch, 1u + u32(b * 3.0));
}

fn cell(c: vec2<u32>) -> u32 {
    var v: array<f32, 6>;
    var top = 0.0;
    var tone = 0.0;
    var hi = 0.0;
    var mat = 0u;
    var gas = 0u;
    var hole = false;
    var sum = 0.0;
    var tsum = 0.0;
    var mats = 0u;
    for (var j = 0u; j < 6u; j++) {
        let s = sample(to_p(c, vec2<f32>(0.25 + 0.5 * f32(j & 1u), (f32(j >> 1u) + 0.5) / 3.0)));
        v[j] = clamp(s.x, 0.0, 1.0);
        sum += v[j];
        tsum += s.y;
        hi = max(hi, s.y);
        gas += u32(s.w == GAS);
        hole = hole || s.w == HOLE;
        mats |= 1u << u32(s.z);
        if (v[j] > top) {
            top = v[j];
            tone = s.y;
            mat = u32(s.z);
        }
    }
    if (top < 0.03) {
        if (gas == 0u && !hole) {
            return star(to_p(c, vec2<f32>(0.5)));
        }
        return 0u;
    }
    // Wholly inside the gas: density ramp.
    if (gas == 6u && mats == (1u << mat)) {
        let slot = select(tone_slot(tsum / 6.0), 7u, hi > 1.0);
        return pack(RAMP[min(9u, u32(sum / 6.0 * 10.0))], slot);
    }
    // Edges: nearest glyph by shape within the material's vocabulary.
    for (var j = 0u; j < 6u; j++) {
        v[j] = pow(v[j] / top, F.contrast) * top;
    }
    let a = vec4<f32>(v[0], v[1], v[2], v[3]);
    let b = vec2<f32>(v[4], v[5]);
    let mask = F.masks[mat];
    var best = 0u;
    var best_d = dot(a, a) + dot(b, b);
    for (var w = 0u; w < 3u; w++) {
        var bits = mask[w];
        while (bits != 0u) {
            let k = w * 32u + countTrailingZeros(bits);
            bits &= bits - 1u;
            let da = shapes[2u * k] - a;
            let db = shapes[2u * k + 1u].xy - b;
            let d = dot(da, da) + dot(db, db);
            if (d < best_d) {
                best_d = d;
                best = k;
            }
        }
    }
    if (best == 0u) {
        return 0u;
    }
    return best | (tone_slot(tone) << 8u);
}
