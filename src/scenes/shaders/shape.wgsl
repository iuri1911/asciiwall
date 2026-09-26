// Field -> glyph by shape (Alex Harri, "ASCII characters are not pixels"):
// sample the field at the centers of a 2x3 grid of cell regions, sharpen the
// 6D ink vector, then pick the nearest glyph shape among the material's glyphs.
// `field(p)` returns (ink 0..1, tone, material 0..3); tone > 1 = highlight.

fn cell(c: vec2<u32>) -> u32 {
    var v: array<f32, 6>;
    var top = 0.0;
    var tone = 0.0;
    var mat = 0u;
    for (var j = 0u; j < 6u; j++) {
        let s = field(to_p(c, vec2<f32>(0.25 + 0.5 * f32(j & 1u), (f32(j >> 1u) + 0.5) / 3.0)));
        v[j] = clamp(s.x, 0.0, 1.0);
        if (v[j] > top) {
            top = v[j];
            tone = s.y;
            mat = u32(s.z);
        }
    }
    if (top < 0.03) {
        return 0u;
    }
    for (var j = 0u; j < 6u; j++) {
        v[j] = pow(v[j] / top, F.contrast) * top;
    }
    let a = vec4<f32>(v[0], v[1], v[2], v[3]);
    let b = vec2<f32>(v[4], v[5]);
    let mask = F.masks[min(mat, 3u)];
    var best = 0u;
    var best_d = dot(a, a) + dot(b, b); // space
    for (var w = 0u; w < 3u; w++) {
        var bits = mask[w];
        while (bits != 0u) {
            let g = w * 32u + countTrailingZeros(bits);
            bits &= bits - 1u;
            let da = shapes[2u * g] - a;
            let db = shapes[2u * g + 1u].xy - b;
            let d = dot(da, da) + dot(db, db);
            if (d < best_d) {
                best_d = d;
                best = g;
            }
        }
    }
    if (best == 0u) {
        return 0u;
    }
    return best | (tone_slot(tone) << 8u);
}
