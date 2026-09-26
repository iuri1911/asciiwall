// traffic — after XorDev's "Traffic" (x.com/XorDev/status/2036188492221305150):
// light trails sweeping to a vanishing point. In ASCII each trail is a stroke of
// characters: shape matching turns the thin ribbons into - / | \ along their
// tangent, headlights and taillights split the theme ramp, and every car is a
// bright pulse (o * @) running along its lane toward or away from you.

const HORIZON: f32 = 0.30;
const SPEED: f32 = 5.0;

// Road centerline (world x) at depth z; the camera drives along it.
fn road(z: f32) -> f32 {
    let w = z + F.t * SPEED;
    let w0 = F.t * SPEED;
    return 2.6 * (sin(w * 0.05) - sin(w0 * 0.05)) + 1.3 * (sin(w * 0.13 + 1.7) - sin(w0 * 0.13 + 1.7)) - 0.7;
}

fn field(p: vec2<f32>) -> vec3<f32> {
    let dv = (p - vec2<f32>(0.0, HORIZON)) * vec2<f32>(0.6, 1.6);
    let glow = 0.00003 / (dot(dv, dv) + 0.00004);
    var best = vec3<f32>(glow, 1.2, 2.0);
    let dy = HORIZON - p.y;
    if (dy <= 0.004) {
        return best;
    }
    let z = 1.0 / dy;
    let u = p.x * z - road(z);
    // Screen-space gradient of u, so distances are perpendicular to the trails.
    let slope = (road(z + 0.01) - road(z)) / 0.01;
    let gv = vec2<f32>(z, (p.x - slope) * z * z);
    let grad = length(gv);
    let n = gv / grad;
    let fade = smoothstep(45.0, 9.0, z);
    for (var i = 0; i < 10; i++) {
        let fi = f32(i);
        let oncoming = i < 5;
        let side = select(1.0, -1.0, oncoming);
        let lane = side * (0.25 + 0.24 * f32(i % 5)) + 0.04 * sin(F.t * 0.3 + fi * 2.1);
        let d = abs(u - lane) / grad;
        let line = stroke(d, n);
        let ph = fract(z * 0.035 + F.t * select(-0.12, 0.3, oncoming) + hash11(fi));
        let car = pow(select(ph, 1.0 - ph, oncoming), 12.0);
        let v = line * (0.62 + 0.6 * car) * fade;
        if (v > best.x) {
            let head = car > 0.6 && d < 0.004;
            let tone = select(0.42 + 0.3 * car, 0.9 + 0.6 * car, oncoming);
            best = vec3<f32>(v, tone, select(0.0, 1.0, head));
        }
    }
    return best;
}
