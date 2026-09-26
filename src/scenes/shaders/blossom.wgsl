// blossom — after the cherry branch in harshitlog's "Shaders" reel
// (x.com/harshitlog/status/2099485977853231443): a branch reaching in from a
// corner, swaying, shedding petals. Three vocabularies: the wood is line
// characters (/ \ | _ ( )), flowers are dense round ones (* @ & % o), and the
// drifting petals are specks (' , . `).

const NODES: u32 = 64u; // heap-indexed binary tree, node 1 = trunk

var<workgroup> tips: array<vec2<f32>, 64>;
var<workgroup> angles: array<f32, 64>;
var<workgroup> lengths: array<f32, 64>;
// How far node k's segment plus its whole subtree reaches from the segment's start.
var<workgroup> reach: array<f32, 64>;

fn node_depth(k: u32) -> u32 {
    return 31u - countLeadingZeros(k);
}

// The trunk enters from beyond the top-right corner.
fn root() -> vec2<f32> {
    return vec2<f32>(0.5 * F.aspect + 0.04, 0.58);
}

// Grow the tree once per workgroup, one level at a time (a node needs its parent).
fn prepare(lid: u32) {
    for (var level = 0u; level < 6u; level++) {
        let k = (1u << level) + lid;
        if (lid < (1u << level)) {
            let d = f32(level);
            let h = hash11(f32(k) * 1.37);
            var start = root();
            var a = PI + 0.42;
            var len = 0.34;
            if (k > 1u) {
                let parent = k >> 1u;
                start = tips[parent];
                let side = select(-1.0, 1.0, (k & 1u) == 1u);
                a = angles[parent] + side * (0.3 + 0.35 * h) - 0.06 * d;
                len = lengths[parent] * (0.64 + 0.18 * h);
            }
            a += 0.018 * d * sin(F.t * 0.8 + d * 0.9 + h * 6.0);
            angles[k] = a;
            lengths[k] = len;
            tips[k] = start + len * vec2<f32>(cos(a), sin(a));
        }
        workgroupBarrier();
    }
    for (var level = 6u; level > 0u; level--) {
        let k = (1u << (level - 1u)) + lid;
        if (lid < (1u << (level - 1u))) {
            var sub = 0.0;
            if (k < NODES / 2u) {
                sub = max(reach[2u * k], reach[2u * k + 1u]);
            }
            reach[k] = lengths[k] + sub;
        }
        workgroupBarrier();
    }
}

fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

fn petals(p: vec2<f32>) -> f32 {
    var ink = 0.0;
    for (var l = 0; l < 2; l++) {
        let fl = f32(l);
        let sc = 0.075 + 0.03 * fl;
        var q = p + F.t * vec2<f32>(0.03 + 0.012 * fl, 0.05 + 0.02 * fl);
        q.x += 0.02 * sin(q.y * 9.0 + fl * 2.0 + F.t * 0.7);
        let id = floor(q / sc);
        let h = hash21(id + fl * 17.0);
        if (h > 0.86) {
            let c = (id + 0.25 + 0.5 * vec2<f32>(hash21(id + 3.1), hash21(id + 8.3))) * sc;
            let d = rot(F.t * (1.0 + 3.0 * h)) * (q - c);
            let e = length(d * vec2<f32>(1.0, 2.3));
            ink = max(ink, smoothstep(0.009, 0.004, e) * (0.6 + 0.4 * h));
        }
    }
    return ink;
}

fn field(p0: vec2<f32>) -> vec3<f32> {
    var p = p0;
    if ((F.seed & 1u) == 1u) {
        p.x = -p.x;
    }
    var best = vec3<f32>(petals(p), 0.62, 2.0);
    // Depth-first walk of the heap-indexed tree, skipping subtrees that can't reach p.
    var k = 1u;
    loop {
        let d = node_depth(k);
        var start = root();
        if (k > 1u) {
            start = tips[k >> 1u];
        }
        var descend = false;
        if (distance(p, start) < reach[k] + 0.04) {
            let dist = seg_dist(p, start, tips[k]);
            if (dist < 0.03) {
                let w = 0.005 * pow(0.6, f32(d));
                let dir = normalize(tips[k] - start);
                let wood = stroke(max(dist - w, 0.0), vec2<f32>(-dir.y, dir.x)) * 0.8;
                if (wood > best.x) {
                    best = vec3<f32>(wood, 0.12 + 0.03 * f32(d), 0.0);
                }
            }
            if (d >= 3u) {
                let h = hash11(f32(k) * 3.7);
                let v = p - tips[k] - 0.012 * vec2<f32>(cos(h * 20.0), sin(h * 20.0));
                let r = length(v);
                if (r < 0.03) {
                    let rad = 0.013 * (0.7 + 0.6 * h) * (0.75 + 0.25 * cos(5.0 * atan2(v.y, v.x) + h * 9.0));
                    let bloom = smoothstep(rad + 0.004, rad - 0.004, r) * 0.8;
                    if (bloom > best.x) {
                        best = vec3<f32>(bloom, 0.7 + 0.55 * (1.0 - r / rad), 1.0);
                    }
                }
            }
            descend = k < NODES / 2u;
        }
        if (descend) {
            k = 2u * k;
            continue;
        }
        // Next sibling, or the sibling of the nearest ancestor that has one.
        while (k > 1u && (k & 1u) == 1u) {
            k = k >> 1u;
        }
        if (k == 1u) {
            break;
        }
        k += 1u;
    }
    return best;
}
