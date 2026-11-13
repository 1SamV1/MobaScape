struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

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

fn sample_scene(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(scene_texture, scene_sampler, uv).rgb;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// Console-style FXAA: edge-direction blur with a luma clamp.
fn fxaa(uv: vec2<f32>, texel: vec2<f32>) -> vec3<f32> {
    let rgbM = sample_scene(uv);
    let lumaM = luma(rgbM);
    let lumaNW = luma(sample_scene(uv + vec2<f32>(-1.0, -1.0) * texel));
    let lumaNE = luma(sample_scene(uv + vec2<f32>(1.0, -1.0) * texel));
    let lumaSW = luma(sample_scene(uv + vec2<f32>(-1.0, 1.0) * texel));
    let lumaSE = luma(sample_scene(uv + vec2<f32>(1.0, 1.0) * texel));

    let lumaMin = min(lumaM, min(min(lumaNW, lumaNE), min(lumaSW, lumaSE)));
    let lumaMax = max(lumaM, max(max(lumaNW, lumaNE), max(lumaSW, lumaSE)));
    let range = lumaMax - lumaMin;
    if (range < max(0.0312, lumaMax * 0.125)) {
        return rgbM;
    }

    var dir = vec2<f32>(
        -((lumaNW + lumaNE) - (lumaSW + lumaSE)),
        (lumaNW + lumaSW) - (lumaNE + lumaSE),
    );
    let dirReduce = max((lumaNW + lumaNE + lumaSW + lumaSE) * 0.03125, 0.0078125);
    let rcpDirMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + dirReduce);
    dir = clamp(dir * rcpDirMin, vec2<f32>(-8.0), vec2<f32>(8.0)) * texel;

    let rgbA = 0.5 * (
        sample_scene(uv + dir * (1.0 / 3.0 - 0.5))
        + sample_scene(uv + dir * (2.0 / 3.0 - 0.5))
    );
    let rgbB = rgbA * 0.5 + 0.25 * (
        sample_scene(uv - dir)
        + sample_scene(uv + dir)
    );
    let lumaB = luma(rgbB);
    if (lumaB < lumaMin || lumaB > lumaMax) {
        return rgbA;
    }
    return rgbB;
}

fn cheap_bloom(uv: vec2<f32>, texel: vec2<f32>) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    let radius = texel * 4.0;
    sum += max(sample_scene(uv + vec2<f32>(radius.x, 0.0)) - 0.82, vec3<f32>(0.0));
    sum += max(sample_scene(uv + vec2<f32>(-radius.x, 0.0)) - 0.82, vec3<f32>(0.0));
    sum += max(sample_scene(uv + vec2<f32>(0.0, radius.y)) - 0.82, vec3<f32>(0.0));
    sum += max(sample_scene(uv + vec2<f32>(0.0, -radius.y)) - 0.82, vec3<f32>(0.0));
    sum += max(sample_scene(uv + radius) - 0.82, vec3<f32>(0.0));
    sum += max(sample_scene(uv - radius) - 0.82, vec3<f32>(0.0));
    sum += max(sample_scene(uv + vec2<f32>(radius.x, -radius.y)) - 0.82, vec3<f32>(0.0));
    sum += max(sample_scene(uv + vec2<f32>(-radius.x, radius.y)) - 0.82, vec3<f32>(0.0));
    return sum * 0.009;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(scene_texture));
    let texel = 1.0 / dimensions;

    var color = fxaa(input.uv, texel);
    color += cheap_bloom(input.uv, texel);

    // Gentle 117HD-style grade: lift exposure, contrast, saturation, and a
    // warm highlight / cool shadow tint.
    color *= 1.08;
    let display = pow(max(color, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
    let graded = (display - vec3<f32>(0.5)) * 1.03 + vec3<f32>(0.5);
    color = pow(max(graded, vec3<f32>(0.0)), vec3<f32>(2.2));
    let l = luma(color);
    color = mix(vec3<f32>(l), color, 1.06);
    color = mix(
        color * vec3<f32>(0.98, 1.0, 1.04),
        color * vec3<f32>(1.03, 1.0, 0.96),
        smoothstep(0.45, 0.85, l)
    );

    let dither = fract(sin(dot(input.uv * dimensions, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    color += (dither - 0.5) / 255.0;
    return vec4<f32>(color, 1.0);
}
