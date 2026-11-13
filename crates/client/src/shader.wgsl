struct Camera {
    view_proj: mat4x4<f32>,
    position: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world: vec3<f32>,
};

@vertex
fn vs(input: VsIn) -> VsOut {
    var output: VsOut;
    output.clip = camera.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.world = input.position;
    return output;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    let dist = length(input.world - camera.position.xyz);
    let fog = clamp((dist - 9000.0) / 28000.0, 0.0, 1.0);
    let sky = vec3<f32>(0.58, 0.72, 0.82);
    let rgb = mix(input.color.rgb, sky, fog * 0.82);
    return vec4<f32>(rgb, 1.0);
}
