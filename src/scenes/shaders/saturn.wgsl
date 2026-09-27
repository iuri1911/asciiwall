// saturn — a ringed planet lit the way a1k0n's donut.c lights its torus (after
// Andy Sloane's write-up: project the surface, dot its normal with the light, map
// luminance to glyph density), but ray-cast analytically: one sphere, one ring
// plane, two moons. Solid cells take measured density ramps — the planet
// ` .:;+*o#%@` with drifting belts and a crisp terminator, the rings `.-=` split by
// the Cassini gap — and edge cells are shape-matched (after Alex Harri) to line
// glyphs such as - _ ( ) ' . The rings pass in front below and behind above, the
// planet shadows them, they band it back; moons are single `o`s over still stars.

const R: f32 = 0.25; // planet radius, screen heights
const PLANET_RAMP = array<u32, 10>(32u, 46u, 58u, 59u, 43u, 42u, 111u, 35u, 37u, 64u); //  .:;+*o#%@
const RING_RAMP = array<u32, 4>(32u, 46u, 45u, 61u); //  .-=

struct Geo {
    c: vec2<f32>, // planet center on screen
    n: vec3<f32>, // ring-plane normal = spin axis
    e1: vec3<f32>, // in-plane basis
    e2: vec3<f32>,
    l: vec3<f32>, // toward the sun
    m: array<vec3<f32>, 2>, // moons, relative to the planet center
};

fn geo() -> Geo {
    var g: Geo;
    g.c = vec2<f32>(-0.1, 0.015);
    // Ring opening angle rocks between ~20 and ~28 degrees; the axis precesses a little.
    let b = 0.42 + 0.07 * sin(F.t * 0.011 + 0.8);
    let psi = 0.22 * sin(F.t * 0.007 + 2.0);
    let roll = -0.2 + 0.05 * sin(F.t * 0.009 + 4.0);
    let n0 = vec3<f32>(sin(b) * sin(psi), cos(b), sin(b) * cos(psi));
    let cr = cos(roll);
    let sr = sin(roll);
    g.n = vec3<f32>(cr * n0.x - sr * n0.y, sr * n0.x + cr * n0.y, n0.z);
    g.e1 = normalize(cross(g.n, vec3<f32>(0.0, 0.0, 1.0)));
    g.e2 = cross(g.n, g.e1);
    g.l = normalize(vec3<f32>(-0.55, 0.42, 0.72));
    // Moon orbits sit close to edge-on so they pass in front of and behind the planet.
    let ph = f32(F.seed % 97u) * 0.71;
    for (var i = 0u; i < 2u; i++) {
        let fi = f32(i);
        let orbit = select(3.3, 2.75, i == 0u) * R;
        let a = F.t * select(0.009, 0.014, i == 0u) + ph * (1.0 + fi) + fi * 2.4;
        let e2 = normalize(g.e2 + select(-0.2, -0.3, i == 0u) * g.n);
        g.m[i] = orbit * (cos(a) * g.e1 + sin(a) * e2);
    }
    return g;
}

// Ring opacity at radius r (planet radii): C ring, B ring, Cassini division, A
// ring. Edges are softened by `w`.
fn ring_opacity(r: f32, w: f32) -> f32 {
    let c = smoothstep(1.22 - w, 1.22 + w, r) * smoothstep(1.52 + w, 1.52 - w, r) * 0.28;
    let b = smoothstep(1.52 - w, 1.52 + w, r) * smoothstep(1.89 + w, 1.89 - w, r) * (0.7 + 0.3 * smoothstep(1.55, 1.8, r));
    let a = smoothstep(2.1 - w, 2.1 + w, r) * smoothstep(2.34 + w, 2.34 - w, r) * 0.62;
    return c + b + a;
}

// Sunlight on the sphere point `s` (relative to the planet center): latitude
// bands drifting at their own speeds, and the rings' shadow. (light, n.l)
fn planet_light(g: Geo, s: vec3<f32>) -> vec2<f32> {
    let n = s / R;
    let ndl = dot(n, g.l);
    if (ndl <= 0.0) {
        return vec2<f32>(0.0);
    }
    let h = dot(n, g.n); // height along the spin axis
    let lon = atan2(dot(n, g.e2), dot(n, g.e1));
    // Belts and zones evenly spaced in height; their edges ripple slowly, each
    // latitude drifting at its own speed.
    let drift = F.t * (0.025 + 0.03 * cos(h * 4.0));
    let wh = h + 0.02 * sin(3.0 * lon - drift) + 0.012 * sin(7.0 * lon + drift * 1.6 + h * 9.0);
    let band = 0.5 + 0.5 * cos(wh * 9.5 + 0.2);
    var albedo = 0.55 + 0.45 * smoothstep(0.3, 0.7, band);
    albedo *= 1.0 - 0.4 * smoothstep(0.8, 0.95, abs(h)); // darker poles
    // Ring shadow: follow the sunbeam back to the ring plane.
    let k = -dot(s, g.n) / dot(g.l, g.n);
    var shade = 1.0;
    if (k > 0.0) {
        let y = s + k * g.l;
        shade = 1.0 - 0.85 * min(ring_opacity(length(y) / R, 0.02), 1.0);
    }
    // A crisp terminator: light starts at a fifth of full, not from zero.
    let lambert = smoothstep(0.0, 0.05, ndl) * (0.2 + 0.8 * pow(ndl, 0.8));
    return vec2<f32>(lambert * albedo * shade, ndl);
}

// One sample: (ink, tone, material, solid). Material 0 planet, 1 ring. `solid`
// marks cells fully inside the disk or a ring band (they take the density ramp)
// and hides the stars.
fn sample(g: Geo, p: vec2<f32>) -> vec4<f32> {
    let q = p - g.c;
    let cs = cell_size();
    let far = -1e3;

    var z_opaque = far;
    var opaque = vec4<f32>(0.0);
    let q2 = dot(q, q);
    if (q2 < R * R) {
        let zs = sqrt(R * R - q2);
        z_opaque = zs;
        let lit = planet_light(g, vec3<f32>(q, zs));
        var tone = 0.08 + 0.9 * lit.x;
        // Highlight only on the thin sunward limb.
        if (lit.y > 0.75 && zs < 0.42 * R) {
            tone = 1.2;
        }
        opaque = vec4<f32>(lit.x, tone, 0.0, 1.0);
    }
    // Ring plane crossing along the view ray.
    let zr = -(q.x * g.n.x + q.y * g.n.y) / g.n.z;
    let x = vec3<f32>(q, zr);
    let rr = length(x) / R;
    if (rr > 1.15 && rr < 2.5 && zr > z_opaque) {
        // Soften edges by about a sixth of a row in ring radius units.
        let op = ring_opacity(rr, 0.18 * cs.y / (R * max(g.n.z, 0.2)));
        // Planet shadow on the rings.
        let b = dot(x, g.l);
        let shadow = 1.0 - smoothstep(R * 1.02, R * 0.94, length(x - b * g.l)) * step(b, 0.0);
        let sunward = dot(normalize(x.xy), normalize(g.l.xy));
        let ink = op * shadow;
        var tone = 0.2 + 0.58 * ink;
        if (rr > 1.76 && rr < 1.89 && sunward > 0.97 && shadow > 0.9) {
            tone = 1.15;
        }
        if (op > 0.5 || opaque.w == 0.0) {
            return vec4<f32>(ink, tone, 1.0, select(0.0, 1.0, op > 0.2));
        }
        // Thin rings over the planet only dim it.
        opaque.x *= 1.0 - 0.6 * op;
    }
    return opaque;
}

// A moon whose center falls in cell `c` and isn't hidden by the planet or a
// dense ring: one `o`, dimmed while eclipsed in the planet's shadow.
fn moon(g: Geo, c: vec2<u32>) -> u32 {
    for (var i = 0u; i < 2u; i++) {
        let m = g.m[i];
        var s = g.c + m.xy;
        if ((F.seed & 1u) == 1u) {
            s.x = -s.x;
        }
        let uv = vec2<f32>(s.x / F.aspect + 0.5, 0.5 - s.y) * vec2<f32>(f32(F.cols), f32(F.rows));
        if (any(vec2<u32>(max(uv, vec2<f32>(0.0))) != c) || any(uv < vec2<f32>(0.0))) {
            continue;
        }
        let q = m.xy;
        let q2 = dot(q, q);
        if (q2 < R * R && sqrt(R * R - q2) > m.z) {
            continue;
        }
        let zr = -(q.x * g.n.x + q.y * g.n.y) / g.n.z;
        if (zr > m.z && ring_opacity(length(vec3<f32>(q, zr)) / R, 0.02) > 0.3) {
            continue;
        }
        let b = dot(m, g.l);
        let lit = 1.0 - 0.85 * smoothstep(R * 1.05, R * 0.9, length(m - b * g.l)) * step(b, 0.0);
        return pack(111u, tone_slot(select(0.55, 0.85, i == 0u) * lit));
    }
    return 0u;
}

// Still stars on a scene-space lattice, so they sit in place on every monitor.
fn star(p: vec2<f32>) -> u32 {
    let cs = cell_size();
    let lat = vec2<f32>(0.045, 0.06);
    let id = floor(p / lat);
    let h = hash21(id);
    if (h < 0.84) {
        return 0u;
    }
    let s = (id + 0.5 + 0.6 * (vec2<f32>(hash21(id + 3.7), hash21(id + 9.1)) - 0.5)) * lat;
    let d = abs(s - p);
    if (d.x > 0.5 * cs.x || d.y > 0.5 * cs.y) {
        return 0u;
    }
    let b = hash21(id + 5.3);
    var ch = 46u; // .
    if (b > 0.93) {
        ch = 42u; // *
    } else if (b > 0.8) {
        ch = 43u; // +
    } else if (b > 0.55) {
        ch = 39u; // '
    }
    return pack(ch, 1u + u32(b * 3.5));
}

fn cell(c: vec2<u32>) -> u32 {
    let g = geo();
    let mo = moon(g, c);
    if (mo != 0u) {
        return mo;
    }
    var v: array<f32, 6>;
    var top = 0.0;
    var tone = 0.0;
    var hi = 0.0;
    var mat = 0u;
    var solid = 0u;
    var sum = 0.0;
    var tsum = 0.0;
    var mats = 0u;
    for (var j = 0u; j < 6u; j++) {
        var p = to_p(c, vec2<f32>(0.25 + 0.5 * f32(j & 1u), (f32(j >> 1u) + 0.5) / 3.0));
        if ((F.seed & 1u) == 1u) {
            p.x = -p.x;
        }
        let s = sample(g, p);
        v[j] = clamp(s.x, 0.0, 1.0);
        sum += v[j];
        tsum += s.y;
        hi = max(hi, s.y);
        solid += u32(s.w > 0.5);
        mats |= 1u << u32(s.z);
        if (v[j] > top) {
            top = v[j];
            tone = s.y;
            mat = u32(s.z);
        }
    }
    if (solid == 0u && top < 0.03) {
        var pc = to_p(c, vec2<f32>(0.5));
        if ((F.seed & 1u) == 1u) {
            pc.x = -pc.x;
        }
        return star(pc);
    }
    if (top < 0.03) {
        return 0u;
    }
    let mean = sum / 6.0;
    let slot = select(tone_slot(tsum / 6.0), 7u, hi > 1.0);
    // Solid interior: measured density ramps.
    if (solid == 6u && mats == (1u << mat)) {
        if (mat == 0u) {
            return pack(PLANET_RAMP[min(9u, u32(mean * 10.0))], slot);
        }
        if (mat == 1u) {
            return pack(RING_RAMP[min(3u, u32(mean * 4.2))], slot);
        }
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
    return best | (select(tone_slot(tone), 7u, tone > 1.0) << 8u);
}
