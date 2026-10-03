// Retro bouncing ball with a phosphor afterimage trail.
//
// This renderer is single-pass (no history/feedback texture), so the trail is
// computed analytically: the ball's motion is deterministic, so we sample its
// position at several past instants and sum a decaying glow for each. CRT-style
// scanlines and a vignette finish the look. `u.seed` shifts the colour, speed,
// and phase so each monitor differs.

struct Uniforms {
    time: f32,
    seed: f32,
    resolution: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var verts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    return vec4<f32>(verts[vi], 0.0, 1.0);
}

// Triangle wave in [0,1] — the bounce curve.
fn tri(x: f32) -> f32 {
    return abs(fract(x) * 2.0 - 1.0);
}

// Ball centre at time `t`, in aspect-corrected space (y in [0,1], x in [0,aspect]).
fn ball_pos(t: f32, aspect: f32, r: f32, seed: f32) -> vec2<f32> {
    let sx = 0.16 + 0.05 * sin(seed);
    let sy = 0.13 + 0.05 * cos(seed * 1.7);
    let x = r + tri(t * sx + seed) * (aspect - 2.0 * r);
    let y = r + tri(t * sy + seed * 0.5) * (1.0 - 2.0 * r);
    return vec2<f32>(x, y);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let res = u.resolution;
    let aspect = res.x / res.y;
    // Aspect-corrected pixel coordinate: y in [0,1], x in [0,aspect].
    let p = frag.xy / res.y;

    let r = 0.2;                // ball radius
    let seed = u.seed;

    var col = vec3<f32>(0.0);
    let steps = 25;
    let balls = 3;

    // Decoupled tail/speed controls:
    //   speed        — ball pace (lower = slower); does NOT change tail length.
    //   tail_seconds — how far back the smear reaches, in ball-time units.
    // The trail is sampled over a fixed window of ball-time, so its spatial
    // length depends only on tail_seconds, not on speed.
    let speed = 0.5;
    let tail_seconds = 2.0;
    let dt = tail_seconds / f32(steps);
    for (var b = 0; b < balls; b = b + 1) {
        // Distinct phase, speed, and colour per ball (golden-angle offset).
        let bseed = seed + f32(b) * 2.39963;
        // Hue slowly drifts over time so the palette keeps evolving. `hue_rate`
        // is radians/sec; bseed keeps the balls offset from each other.
        let hue_rate = 0.05;
        let hue = bseed + u.time * hue_rate;
        let tint = 0.55 + 0.45 * cos(hue + vec3<f32>(0.0, 2.1, 4.2));

        var glow = 0.0;
        var core = 0.0;
        for (var i = 0; i < steps; i = i + 1) {
            let age = f32(i);
            let t = u.time * speed - age * dt;  // step back over a fixed ball-time window
            let bp = ball_pos(t, aspect, r, bseed);
            let d = distance(p, bp);
            let decay = pow(0.75, age);          // older = dimmer
            glow += exp(-d * d / (r * r) * 4.2) * decay;
            if (i == 0) {
                core = smoothstep(r, 0, d);  // crisp head
            }
        }
        col += tint * (glow * 0.55) + vec3<f32>(core);
    }

    return vec4<f32>(col, 1.0);
}
