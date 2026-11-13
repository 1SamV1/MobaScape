struct SceneGlobals {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    ambient_sky: vec4<f32>,
    ambient_ground: vec4<f32>,
    fog_color: vec4<f32>,
    fog_params: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    sun_screen: vec4<f32>,
    light_view_proj: mat4x4<f32>,
    shadow_params: vec4<f32>,
    tuning: vec4<f32>,
    lights: array<vec4<f32>, 48>,
    light_count: vec4<u32>,
};

@group(0) @binding(0) var<uniform> scene: SceneGlobals;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[vertex_index];
    var output: VsOut;
    output.clip = vec4<f32>(p, 1.0, 1.0);
    output.uv = p * 0.5 + 0.5;
    return output;
}

fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 4; i = i + 1) {
        value += noise2(q) * amp;
        q = q * 2.03 + vec2<f32>(13.7, 7.1);
        amp *= 0.5;
    }
    return value;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    let t = clamp(input.uv.y, 0.0, 1.0);
    var col = mix(scene.sky_horizon.rgb, scene.sky_top.rgb, pow(t, 0.62));

    // Blend the bottom of the sky into the fog/horizon color so the distant
    // ground plane does not end in a hard line.
    let haze = 1.0 - smoothstep(0.0, 0.30, t);
    col = mix(col, scene.fog_color.rgb, haze * 0.75);

    let cloud_uv = input.uv * vec2<f32>(5.0, 2.6)
        + vec2<f32>(scene.tuning.z * 0.006, scene.tuning.z * 0.002);
    let cloud = smoothstep(0.52, 0.72, fbm(cloud_uv))
        * smoothstep(0.06, 0.34, t)
        * (1.0 - smoothstep(0.72, 1.0, t));
    col = mix(col, vec3<f32>(0.82, 0.88, 0.92), cloud * 0.42);

    let p = input.uv * 2.0 - 1.0;
    let d = length(p - scene.sun_screen.xy);
    let disk = smoothstep(0.045, 0.018, d) * scene.sun_screen.z;
    col = mix(col, scene.sun_color.rgb * 2.2, disk);
    col += scene.sun_color.rgb * exp(-d * 7.0) * 0.14 * scene.sun_screen.z;
    return vec4<f32>(col, 1.0);
}
