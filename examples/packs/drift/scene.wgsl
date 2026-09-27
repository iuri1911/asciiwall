// Specimen for the pack contract. A slow horizontal drift; Natural already
// uses pulse = 0.8, and the user's Slow/Fast steps multiply that.
fn field(p: vec2<f32>) -> vec3<f32> {
    let v = 0.5 + 0.5 * sin(p.x * 6.0 - F.t);
    return vec3<f32>(v * 0.4, v, 0.0);
}
