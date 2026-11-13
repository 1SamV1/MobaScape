struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var hdr_texture: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;

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
    output.uv = p * vec2<f32>(0.5, -0.5) + 0.5;
    return output;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(hdr_texture, hdr_sampler, input.uv).rgb;
    let brightness = max(max(c.r, c.g), c.b);
    let contribution = smoothstep(1.0, 2.0, brightness);
    return vec4<f32>(c * contribution, 1.0);
}
