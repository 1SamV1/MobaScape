struct BloomUniform {
    direction: vec2<f32>,
    texel: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var input_texture: texture_2d<f32>;
@group(0) @binding(1) var input_sampler: sampler;
@group(0) @binding(2) var<uniform> params: BloomUniform;

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
    let step = params.direction * params.texel;
    var sum = textureSample(input_texture, input_sampler, input.uv).rgb * 0.2270270270;
    sum += textureSample(input_texture, input_sampler, input.uv + step * 1.0).rgb * 0.1945945946;
    sum += textureSample(input_texture, input_sampler, input.uv - step * 1.0).rgb * 0.1945945946;
    sum += textureSample(input_texture, input_sampler, input.uv + step * 2.0).rgb * 0.1216216216;
    sum += textureSample(input_texture, input_sampler, input.uv - step * 2.0).rgb * 0.1216216216;
    sum += textureSample(input_texture, input_sampler, input.uv + step * 3.0).rgb * 0.0540540541;
    sum += textureSample(input_texture, input_sampler, input.uv - step * 3.0).rgb * 0.0540540541;
    sum += textureSample(input_texture, input_sampler, input.uv + step * 4.0).rgb * 0.0162162162;
    sum += textureSample(input_texture, input_sampler, input.uv - step * 4.0).rgb * 0.0162162162;
    return vec4<f32>(sum, 1.0);
}
