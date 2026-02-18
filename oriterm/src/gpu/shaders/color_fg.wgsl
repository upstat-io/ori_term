// Color foreground shader: atlas-sampled color emoji quads.
//
// Same vertex pulling pattern as the foreground shader. The fragment shader
// samples an Rgba8Unorm atlas texture array and outputs the color directly
// without fg_color tinting — color emoji carry their own RGBA data.

struct Uniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniform;

@group(1) @binding(0)
var color_atlas_texture: texture_2d_array<f32>;

@group(1) @binding(1)
var color_atlas_sampler: sampler;

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
    @location(0) tex_coord: vec2<f32>,
    @location(1) @interpolate(flat) atlas_page: u32,
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
    out.tex_coord = tex_coord;
    out.atlas_page = instance.atlas_page;
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample RGBA directly — color emoji data is premultiplied.
    return textureSample(color_atlas_texture, color_atlas_sampler, input.tex_coord, input.atlas_page);
}
