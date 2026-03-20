// blit.wgsl — Copies an offscreen texture to the swapchain surface.

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(src_texture));
    let uv = frag_coord.xy / dims;
    return textureSample(src_texture, src_sampler, uv);
}
