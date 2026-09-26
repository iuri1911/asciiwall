// fractal — after Yohei Nishitsuji's #つぶやきGLSL folded fractals
// (x.com/YoheiNishitsuji): space folded, twisted and scaled over and over.
// Here the folds build a floating Sierpinski relic whose twist drifts over
// time, so it keeps morphing. Transposed for ASCII the way donut.c lights its
// torus: one ray per character cell, the march stops at the size of a cell
// (sub-cell detail would only alias into noise), and light picks from the
// same ramp donut.c uses, . , - ~ : ; = ! * # $ @

const STEPS: i32 = 64;
const ITER: i32 = 6;
const RAMP: array<u32, 12> = array<u32, 12>(46u, 44u, 45u, 126u, 58u, 59u, 61u, 33u, 42u, 35u, 36u, 64u);

fn de(p0: vec3<f32>) -> f32 {
    var z = p0;
    let twist = rot(0.25 * sin(F.t * 0.05) + 0.1);
    let tilt = rot(0.18 * cos(F.t * 0.037));
    for (var n = 0; n < ITER; n++) {
        if (z.x + z.y < 0.0) {
            z = vec3<f32>(-z.y, -z.x, z.z);
        }
        if (z.x + z.z < 0.0) {
            z = vec3<f32>(-z.z, z.y, -z.x);
        }
        if (z.y + z.z < 0.0) {
            z = vec3<f32>(z.x, -z.z, -z.y);
        }
        let xy = twist * z.xy;
        z = vec3<f32>(xy.x, xy.y, z.z);
        let yz = tilt * z.yz;
        z = vec3<f32>(z.x, yz.x, yz.y);
        z = z * 2.0 - vec3<f32>(1.0);
    }
    return (length(z) - 1.2) * exp2(-f32(ITER));
}

// Object-space orientation: a slow tumble.
fn spin(q: vec3<f32>) -> vec3<f32> {
    let xz = rot(F.t * 0.07) * q.xz;
    let r = vec3<f32>(xz.x, q.y, xz.y);
    let yz = rot(0.55 + 0.15 * sin(F.t * 0.04)) * r.yz;
    return vec3<f32>(r.x, yz.x, yz.y);
}

// (light 0..1, tone)
fn shade(p: vec2<f32>) -> vec2<f32> {
    let eye = vec3<f32>(0.0, 0.0, -6.2);
    let dir = normalize(vec3<f32>(p, 1.25));
    let cone = cell_size().y * 0.5;
    var g = 2.0;
    var hit = false;
    var steps = 0.0;
    for (var i = 0; i < STEPS; i++) {
        let d = de(spin(eye + dir * g));
        if (d < cone * g * 0.4) {
            hit = true;
            break;
        }
        g += d;
        steps += 1.0;
        if (g > 10.0) {
            break;
        }
    }
    if (!hit) {
        // A faint halo so the relic floats in something.
        return vec2<f32>(0.0);
    }
    let x = spin(eye + dir * g);
    // Normals from a cell-sized footprint: shading stays smooth at text resolution.
    let e = vec2<f32>(cone * g, 0.0);
    let n = normalize(vec3<f32>(
        de(x + e.xyy) - de(x - e.xyy),
        de(x + e.yxy) - de(x - e.yxy),
        de(x + e.yyx) - de(x - e.yyx)
    ));
    let light = normalize(spin(vec3<f32>(-0.6, 0.7, -0.5)));
    let diffuse = max(dot(n, light), 0.0);
    let rim = pow(1.0 - abs(dot(n, spin(-dir))), 3.0);
    let occlusion = 1.0 - steps / f32(STEPS);
    let light_level = (0.08 + 0.72 * diffuse + 0.25 * rim) * occlusion;
    return vec2<f32>(light_level, 0.2 + 0.85 * diffuse + 0.35 * rim);
}

fn cell(c: vec2<u32>) -> u32 {
    let s = shade(to_p(c, vec2<f32>(0.5)));
    if (s.x < 0.02) {
        return 0u;
    }
    let ramp = RAMP;
    return pack(ramp[u32(clamp(s.x, 0.0, 1.0) * 11.99)], tone_slot(s.y));
}
