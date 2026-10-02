// Classic "plasma" screensaver effect. The vertex stage emits one oversized
// triangle covering the screen; all the work happens per-pixel in fs_main.

struct Uniforms {
    time: f32,
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

    var v = sin(uv.x * 10.0 + t);
    v += sin((uv.y * 10.0 + t) * 0.5);
    v += sin((uv.x * 10.0 + uv.y * 10.0 + t) * 0.5);

    let cx = uv.x + 0.5 * sin(t * 0.3);
    let cy = uv.y + 0.5 * cos(t * 0.2);
    v += sin(sqrt(100.0 * (cx * cx + cy * cy) + 1.0) + t);

    let pi = 3.14159265;
    let col = vec3<f32>(
        sin(v * pi),
        sin(v * pi + 2.094),
        sin(v * pi + 4.188),
    ) * 0.5 + 0.5;

    return vec4<f32>(col, 1.0);
}
