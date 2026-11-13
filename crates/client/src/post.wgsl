struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var bloom_texture: texture_2d<f32>;
@group(0) @binding(2) var scene_sampler: sampler;

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

fn tonemap(c: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c2 = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((c * (a * c + b)) / (c * (c2 * c + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn composite(uv: vec2<f32>) -> vec3<f32> {
    let scene = textureSample(scene_texture, scene_sampler, uv).rgb;
    let bloom = textureSample(bloom_texture, scene_sampler, uv).rgb;
    return tonemap(scene + bloom * 0.45);
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.299, 0.587, 0.114));
}

// Console-style FXAA on the tonemapped composite.
fn fxaa(uv: vec2<f32>, texel: vec2<f32>) -> vec3<f32> {
    let rgbM = composite(uv);
    let lumaM = luma(rgbM);
    let lumaNW = luma(composite(uv + vec2<f32>(-1.0, -1.0) * texel));
    let lumaNE = luma(composite(uv + vec2<f32>(1.0, -1.0) * texel));
    let lumaSW = luma(composite(uv + vec2<f32>(-1.0, 1.0) * texel));
    let lumaSE = luma(composite(uv + vec2<f32>(1.0, 1.0) * texel));

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
        composite(uv + dir * (1.0 / 3.0 - 0.5))
        + composite(uv + dir * (2.0 / 3.0 - 0.5))
    );
    let rgbB = rgbA * 0.5 + 0.25 * (
        composite(uv - dir)
        + composite(uv + dir)
    );
    let lumaB = luma(rgbB);
    if (lumaB < lumaMin || lumaB > lumaMax) {
        return rgbA;
    }
    return rgbB;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    let dimensions = vec2<f32>(textureDimensions(scene_texture));
    let texel = 1.0 / dimensions;

    var color = fxaa(input.uv, texel);

    // Gentle grade after tonemapping, in display space.
    color *= 0.98;
    color = (color - vec3<f32>(0.5)) * 1.03 + vec3<f32>(0.5);
    let l = luma(color);
    color = mix(vec3<f32>(l), color, 1.06);
    color = mix(
        color * vec3<f32>(0.98, 1.0, 1.04),
        color * vec3<f32>(1.03, 1.0, 0.96),
        smoothstep(0.45, 0.85, l)
    );

    // Dota style vignette: the corners fall off so the eye is pulled to the
    // centre of the fight.
    let centred = input.uv - vec2<f32>(0.5);
    let vignette = 1.0 - smoothstep(0.42, 0.92, length(centred * vec2<f32>(1.05, 1.25)));
    color *= mix(0.72, 1.0, vignette);

    // Slight filmic contrast for the darker, moodier look.
    color = clamp((color - vec3<f32>(0.5)) * 1.05 + vec3<f32>(0.5), vec3<f32>(0.0), vec3<f32>(1.0));

    let dither = fract(sin(dot(input.uv * dimensions, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    color += (dither - 0.5) / 255.0;
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
