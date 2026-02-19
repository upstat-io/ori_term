// Subpixel foreground shader: LCD per-channel alpha blending.
//
// Samples an Rgba8Unorm atlas where R/G/B channels contain independent
// subpixel coverage masks (from swash Format::Subpixel). Each color
// channel is blended independently: mix(bg, fg, mask_channel). This
// achieves ~3x effective horizontal resolution on LCD displays.

struct Uniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniform;

@group(1) @binding(0)
var subpixel_atlas_texture: texture_2d_array<f32>;

@group(1) @binding(1)
var subpixel_atlas_sampler: sampler;

struct InstanceInput {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) fg_color: vec4<f32>,
    @location(4) bg_color: vec4<f32>,
    @location(5) kind: u32,
    @location(6) atlas_page: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fg_color: vec4<f32>,
    @location(1) bg_color: vec4<f32>,
    @location(2) tex_coord: vec2<f32>,
    @location(3) @interpolate(flat) atlas_page: u32,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    // TriangleStrip corners: 0=TL, 1=TR, 2=BL, 3=BR.
    let corner = vec2<f32>(
        f32(vertex_index & 1u),
        f32((vertex_index >> 1u) & 1u),
    );

    let px = instance.pos + instance.size * corner;

    // Pixel to NDC.
    let ndc = vec2<f32>(
        px.x / uniforms.screen_size.x * 2.0 - 1.0,
        1.0 - px.y / uniforms.screen_size.y * 2.0,
    );

    // UV from atlas: origin + extent * corner.
    let tex_coord = instance.uv.xy + instance.uv.zw * corner;

    var out: VertexOutput;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.fg_color = instance.fg_color;
    out.bg_color = instance.bg_color;
    out.tex_coord = tex_coord;
    out.atlas_page = instance.atlas_page;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample per-channel coverage mask from the subpixel atlas.
    // R/G/B contain independent subpixel coverage (0.0–1.0).
    let mask = textureSample(subpixel_atlas_texture, subpixel_atlas_sampler, input.tex_coord, input.atlas_page);

    let fg = input.fg_color;
    let bg = input.bg_color;

    // Per-channel compositing: each channel blended independently.
    let r = mix(bg.r, fg.r, mask.r);
    let g = mix(bg.g, fg.g, mask.g);
    let b = mix(bg.b, fg.b, mask.b);

    // Overall alpha is the maximum channel coverage.
    let a = max(mask.r, max(mask.g, mask.b)) * fg.a;

    // Output in premultiplied alpha for correct compositing.
    return vec4<f32>(r * a, g * a, b * a, a);
}
