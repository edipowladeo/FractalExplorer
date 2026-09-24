@group(0) @binding(0) var tile_texture: texture_2d<f32>;
@group(0) @binding(1) var tile_sampler: sampler;
@fragment
fn main(@location(0) uv: vec2<f32>, @location(1) opacity: f32) -> @location(0) vec4<f32> {
    let color = textureSample(tile_texture, tile_sampler, uv);
    return vec4<f32>(color.rgb, color.a * opacity);
}
