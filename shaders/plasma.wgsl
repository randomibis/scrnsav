// Classic "plasma" screensaver effect. The vertex stage emits one oversized
// triangle covering the screen; all the work happens per-pixel in fs_main.

struct Uniforms {
    time: f32,
    // Per-monitor phase offset so each screen shows a different variation.
    seed: f32,
    resolution: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    // Fullscreen triangle: three verts that extend past the viewport.
    var verts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    return vec4<f32>(verts[vi], 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = frag.xy / u.resolution;
    let t = u.time;
    let s = u.seed;

    var v = sin(uv.x * 10.0 + t + s);
    v += sin((uv.y * 10.0 + t) * 0.5 + s * 1.3);
    v += sin((uv.x * 10.0 + uv.y * 10.0 + t) * 0.5 + s);

    let cx = uv.x + 0.5 * sin(t * 0.3 + s);
    let cy = uv.y + 0.5 * cos(t * 0.2 + s * 0.7);
    v += sin(sqrt(100.0 * (cx * cx + cy * cy) + 1.0) + t);

    let pi = 3.14159265;
    let col = vec3<f32>(
        sin(v * pi + s),
        sin(v * pi + 2.094 + s),
        sin(v * pi + 4.188 + s),
    ) * 0.5 + 0.5;

    // Mute the palette: pull each pixel toward its own grey (luminance) to
    // drop saturation, then scale brightness down so nothing reads as neon.
    let grey = dot(col, vec3<f32>(0.299, 0.587, 0.114));
    let muted = mix(vec3<f32>(grey), col, 0.45) * 0.05;

    return vec4<f32>(muted, 1.0);
}
