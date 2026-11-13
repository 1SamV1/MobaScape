struct Light {
    position: vec4<f32>,
    color: vec4<f32>,
    params: vec4<f32>,
};

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
    vision: vec4<f32>,
    lights: array<Light, 16>,
    light_count: vec4<u32>,
};

@group(0) @binding(0) var<uniform> scene: SceneGlobals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;
@group(0) @binding(3) var terrain_textures: texture_2d_array<f32>;
@group(0) @binding(4) var terrain_sampler: sampler;

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
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) ao: f32,
    @location(5) @interpolate(flat) material: u32,
};

@vertex
fn vs(input: VsIn) -> VsOut {
    var output: VsOut;
    output.clip = scene.view_proj * vec4<f32>(input.position, 1.0);
    output.color = input.color;
    output.world = input.position;
    output.normal = input.normal;
    output.uv = input.uv;
    output.ao = input.ao;
    output.material = input.material;
    return output;
}

fn dynamic_lighting(input: VsOut) -> vec3<f32> {
    var sum = vec3<f32>(0.0);
    let N = normalize(input.normal);
    let count = min(scene.light_count.x, 16u);
    for (var i = 0u; i < count; i = i + 1u) {
        let light = scene.lights[i];
        let to_light = light.position.xyz - input.world;
        let dist = length(to_light);
        let atten = clamp(1.0 - dist / max(light.params.x, 1.0), 0.0, 1.0);
        let lndotl = max(dot(N, normalize(to_light)), 0.0);
        sum += light.color.rgb * light.color.a * atten * atten * lndotl;
    }
    return sum;
}

fn shadow_factor(input: VsOut) -> f32 {
    if (scene.shadow_params.w < 0.5) {
        return 1.0;
    }
    let coord = scene.light_view_proj * vec4<f32>(input.world, 1.0);
    let proj = coord.xyz / coord.w;
    let uv = vec2<f32>(proj.x * 0.5 + 0.5, -proj.y * 0.5 + 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0))
        || proj.z < 0.0 || proj.z > 1.0) {
        return 1.0;
    }
    let texel = scene.shadow_params.x;
    let bias = scene.shadow_params.y;
    // The comparison sampler's linear filter provides a cheap 2x2 PCF.
    return textureSampleCompare(shadow_map, shadow_sampler, uv, proj.z - bias);
}

fn lighting(input: VsOut) -> vec3<f32> {
    let V = normalize(scene.camera_pos.xyz - input.world);
    var N = normalize(input.normal);
    // OSRS models are single sided and a few ship with inverted normals.
    // Flipping the normal towards the camera keeps the face the player can
    // actually see lit instead of black.
    if (dot(N, V) < 0.0) {
        N = -N;
    }
    let L = normalize(scene.sun_dir.xyz);

    var ndotl = max(dot(N, L), 0.0);
    if (scene.tuning.y > 0.0) {
        ndotl = floor(ndotl * scene.tuning.y + 0.5) / scene.tuning.y;
    }

    let hemi = mix(
        scene.ambient_ground.rgb,
        scene.ambient_sky.rgb,
        N.y * 0.5 + 0.5
    );

    let sun = scene.sun_color.rgb * 0.82 * shadow_factor(input);
    // Camera facing fill keeps the fronts of models readable even when the
    // sun is behind them, which a top down MOBA camera hits constantly.
    let unit_fill = select(0.26, 0.48, input.material == 3u);
    let fill = max(dot(N, V), 0.0) * unit_fill;
    let ambient = select(1.22, 1.30, input.material == 3u);
    var lit = albedo(input) * (hemi * ambient + sun * ndotl + fill) * input.ao * 1.04;

    lit += input.color.rgb * dynamic_lighting(input);
    return lit;
}

fn fog_factor(input: VsOut) -> f32 {
    let dist = length(input.world - scene.camera_pos.xyz);
    let base = 1.0 - exp(-scene.fog_params.x * dist);
    let height = exp(
        -max(input.world.y - scene.fog_params.z, 0.0) * scene.fog_params.y
    );
    return clamp(base + height * scene.fog_params.w, 0.0, 0.9);
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

fn albedo(input: VsOut) -> vec3<f32> {
    var c = input.color.rgb;
    if (input.material == 4u) {
        let uv = input.world.xz / 384.0;
        let grass = textureSample(terrain_textures, terrain_sampler, uv, 0).rgb;
        let dirt = textureSample(terrain_textures, terrain_sampler, uv, 1).rgb;
        let stone = textureSample(terrain_textures, terrain_sampler, uv, 2).rgb;
        let sand = textureSample(terrain_textures, terrain_sampler, uv, 3).rgb;
        let water = textureSample(terrain_textures, terrain_sampler, uv, 4).rgb;

        // Classify in display space: linear values compress saturation and
        // make grass look like stone.
        let d = pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
        let green = max(d.g - d.r, 0.0) + max(d.g - d.b, 0.0);
        let brown = max(d.r - d.g, 0.0) * 2.0 + max(d.g - d.b, 0.0) * 0.15;
        let blue = max(d.b - d.g, 0.0) * 2.0 + max(d.b - d.r, 0.0);
        let sat = max(max(d.r, d.g), d.b) - min(min(d.r, d.g), d.b);
        let gray = (1.0 - smoothstep(0.05, 0.25, sat))
            * (1.0 - smoothstep(0.12, 0.30, d.r - d.b));
        let sandy = max(d.r - d.b, 0.0) * max(d.g - d.b, 0.0) * 4.0;
        let total = max(green + brown + blue + gray + sandy, 0.0001);
        var tex = grass * (green / total)
            + dirt * (brown / total)
            + stone * (gray / total)
            + sand * (sandy / total)
            + water * (blue / total);

        // Lanes are baked into the terrain: the vertex alpha carries the
        // road mask and the OSRS road sprites are blended into the ground
        // so there is no ribbon hovering over the terrain.
        let lane = clamp(input.color.a, 0.0, 1.0);
        if (lane > 0.002) {
            let road_uv = input.world.xz / 192.0;
            let road = textureSample(terrain_textures, terrain_sampler, road_uv, 1).rgb;
            let gravel = textureSample(terrain_textures, terrain_sampler, road_uv * 0.63, 5).rgb;
            tex = mix(tex, mix(road, gravel, 0.45), lane * 0.94);
        }

        // The sprite carries the palette; the vertex colour only nudges it
        // so the ground never blows out into a flat highlighter band.
        let tint = 0.86 + c * 1.05;
        let p = input.world.xz / 128.0;
        let fine = noise2(p * 7.0) * 0.55 + noise2(p * 19.0) * 0.30;
        c = tex * tint * (0.90 + fine * 0.16);
    }
    return c;
}

@fragment
fn fs(input: VsOut) -> @location(0) vec4<f32> {
    let flags = u32(scene.tuning.w);
    let debug_view = flags & 15u;

    if (debug_view == 1u) {
        return vec4<f32>(albedo(input), 1.0);
    }
    if (debug_view == 2u) {
        return vec4<f32>(normalize(input.normal) * 0.5 + 0.5, 1.0);
    }
    if (debug_view == 3u) {
        return vec4<f32>(lighting(input), 1.0);
    }
    if (debug_view == 4u) {
        return vec4<f32>(vec3<f32>(fog_factor(input)), 1.0);
    }
    if (debug_view == 5u) {
        return vec4<f32>(dynamic_lighting(input), 1.0);
    }
    if (debug_view == 6u) {
        return vec4<f32>(vec3<f32>(shadow_factor(input)), 1.0);
    }
    if (debug_view == 7u) {
        return vec4<f32>(vec3<f32>(input.ao), 1.0);
    }

    // Unlit geometry: fog-of-war, world-space bars, rings, and markers.
    if (input.material == 1u) {
        return input.color;
    }

    // Classic baseline for A/B comparison.
    if ((flags & 256u) != 0u) {
        let dist = length(input.world - scene.camera_pos.xyz);
        let fog = clamp((dist - 9000.0) / 28000.0, 0.0, 1.0);
        let sky = vec3<f32>(0.296, 0.478, 0.638);
        return vec4<f32>(mix(input.color.rgb, sky, fog * 0.82), 1.0);
    }

    var lit = lighting(input);
    // Fog of war: a consistent lateral circle around the hero. This is a
    // shader term rather than a mesh, so it never fights with terrain z
    // levels and the boundary is a smooth radius instead of a tile edge.
    let los_distance = length(input.world.xz - scene.vision.xz);
    let los = 1.0 - smoothstep(scene.vision.w * 0.80, scene.vision.w * 1.30, los_distance);
    lit = mix(lit * 0.42, lit, los);
    let fog = fog_factor(input);
    let sun_amount = max(
        dot(normalize(input.world - scene.camera_pos.xyz), normalize(scene.sun_dir.xyz)),
        0.0
    );
    let fog_tint = mix(scene.fog_color.rgb, scene.sun_color.rgb, sun_amount * 0.15);
    lit = mix(lit, fog_tint, fog);
    return vec4<f32>(lit, 1.0);
}
