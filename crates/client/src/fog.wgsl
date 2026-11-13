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

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) ao: f32,
    @location(5) material: u32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs(input: VsIn) -> VsOut {
    var output: VsOut;
    output.clip = scene.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.world = input.position;
    return output;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    let noise = fract(sin(dot(input.world.xz, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    // Black with the coverage in alpha: the pipeline multiplies the scene by
    // (1 - alpha), so unseen ground reads as shadow rather than a blue tint.
    let alpha = clamp(input.color.a * (0.94 + noise * 0.06), 0.0, 1.0);
    return vec4<f32>(0.0, 0.0, 0.0, alpha);
}
