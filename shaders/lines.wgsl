// Retro vector lines. Two invisible balls bounce around; a glowing line is drawn
// between them. Like the ball shader, there's no feedback buffer — the fading
// "ribbon" behind the line is computed analytically by drawing the line at a
// series of past instants, each dimmer and tinted by its own point in the colour
// cycle, so the line's colour drifts gradually. CRT scanlines + vignette finish
// the look. `u.seed` varies colour/speed/phase per monitor.

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

// Endpoint position at time `t`, in aspect-corrected space.
fn ball_pos(t: f32, aspect: f32, m: f32, seed: f32) -> vec2<f32> {
    let sx = 0.15 + 0.06 * sin(seed);
    let sy = 0.12 + 0.06 * cos(seed * 1.7);
    let x = m + tri(t * sx + seed) * (aspect - 2.0 * m);
    let y = m + tri(t * sy + seed * 0.5) * (1.0 - 2.0 * m);
    return vec2<f32>(x, y);
}

// Distance from point p to the segment a--b.
fn seg_dist(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / (dot(ba, ba) + 1e-6), 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let res = u.resolution;
    let aspect = res.x / res.y;
    let p = frag.xy / res.y;      // aspect-corrected: y in [0,1], x in [0,aspect]

    let m = 0.0;                  // points, not balls: endpoints reach the edges
    let seed = u.seed;
    let other = seed + 2.39963;   // second endpoint: golden-angle offset

    var col = vec3<f32>(0.0);
    let segments = 48;            // afterimage ribbon length
    for (var i = 0; i < segments; i = i + 1) {
        let age = f32(i);
        let t = u.time - age * 0.06;              // step back in time
        let a = ball_pos(t, aspect, m, seed);
        let b = ball_pos(t, aspect, m, other);
        let d = seg_dist(p, a, b);

        let decay = pow(0.90, age);               // older = dimmer
        let glow = exp(-d * d / 4e-6)             // sharp core (~thin line)
                 + 0.12 * exp(-d * d / 4e-4);     // soft halo

        // Colour cycles with time, so the ribbon is a gradient.
        let hue = t * 0.35 + seed;
        let color = 0.5 + 0.5 * cos(hue + vec3<f32>(0.0, 2.094, 4.188));

        col += color * glow * decay;
    }

    return vec4<f32>(col, 1.0);
}
