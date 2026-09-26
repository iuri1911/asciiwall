// aurora — XorDev's turbulence (stacked sine octaves warping space, see
// mini.gmshaders.com) folded into auroral ribbons over a mountain lake.
// ASCII reading: curtains are vertical strokes (| ! : '), the ridge is drawn
// with / \ _ ^, and the lake echoes the sky using only ~ - = _ . ,

const LAKE: f32 = -0.2;

fn ridge(x: f32) -> f32 {
    return LAKE + 0.01 + 0.11 * noise(vec2<f32>(x * 1.6, 1.3)) + 0.04 * noise(vec2<f32>(x * 5.3, 9.1))
        + 0.015 * noise(vec2<f32>(x * 17.0, 3.7));
}

fn sky(p: vec2<f32>) -> vec3<f32> {
    // Turbulence: each octave pushes x along a sine of y and vice versa.
    var q = p;
    var f = 1.3;
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        q.x += 0.3 * sin(q.y * f * 0.8 + F.t * 0.09 + fi * 1.7) / f;
        q.y += 0.08 * sin(q.x * f * 1.4 - F.t * 0.06 + fi * 2.3) / f;
        f *= 1.55;
    }
    var out = vec3<f32>(0.0, 0.0, 0.0);
    for (var k = 0; k < 3; k++) {
        let fk = f32(k);
        let base = 0.03 + 0.1 * fk + 0.09 * sin(q.x * (2.1 + 0.4 * fk) + fk * 2.1 + F.t * 0.05)
            + 0.035 * sin(q.x * (5.3 - fk) - F.t * 0.08 + fk);
        let h = q.y - base;
        if (h < -0.02) {
            continue;
        }
        let env = exp(-max(h, 0.0) * (11.0 + 4.0 * fk)) * smoothstep(-0.015, 0.0, h);
        let rays = 0.25 + 0.75 * pow(noise(vec2<f32>(q.x * (48.0 + 11.0 * fk) + fk * 13.0, F.t * 0.25)), 2.0);
        let sheet = smoothstep(0.25, 0.7, noise(vec2<f32>(q.x * 0.7 + fk * 5.0 + F.t * 0.02, 1.0 + fk)));
        let v = env * rays * sheet * (0.9 - 0.2 * fk);
        if (v > out.x) {
            out = vec3<f32>(v, clamp(0.95 - h * 1.4 - 0.15 * fk, 0.2, 1.0), 0.0);
        }
    }
    if (out.x < 0.08) {
        let cs = cell_size();
        let cell = cs * vec2<f32>(3.0, 2.0);
        let id = floor(p / cell);
        let s = hash21(id);
        let dd = abs(p - (id + 0.5) * cell) / cs;
        if (s > 0.94 && dd.x < 0.3 && dd.y < 0.2) {
            out = vec3<f32>(0.4 + 0.3 * sin(F.t * 1.3 + s * 80.0), 0.5, 3.0);
        }
    }
    return out;
}

fn land(p: vec2<f32>) -> vec3<f32> {
    let r = ridge(p.x);
    if (p.y <= r) {
        return vec3<f32>(0.0);
    }
    // The ridge line: slope decides the stroke's normal.
    let slope = (ridge(p.x + 0.002) - r) / 0.002;
    let n = normalize(vec2<f32>(-slope, 1.0));
    let edge = stroke((p.y - r) * n.y, n) * 0.7;
    let s = sky(p);
    if (edge > s.x) {
        return vec3<f32>(edge, 0.2, 1.0);
    }
    return s;
}

fn field(p: vec2<f32>) -> vec3<f32> {
    if (p.y >= LAKE) {
        return land(p);
    }
    let depth = LAKE - p.y;
    let wobble = 0.005 * sin(p.y * 240.0 + F.t * 1.6) + 0.004 * sin(p.x * 30.0 - F.t);
    let m = land(vec2<f32>(p.x + wobble, LAKE + depth * 1.8));
    let ripple = 0.6 + 0.4 * sin(p.y * 300.0 - F.t * 2.0 + 3.0 * noise(vec2<f32>(p.x * 8.0, p.y * 40.0)));
    return vec3<f32>(m.x * 0.7 * ripple * exp(-depth * 1.5), m.y * 0.8, 2.0);
}
